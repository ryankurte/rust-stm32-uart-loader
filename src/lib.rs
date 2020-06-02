//! STM32 Serial Bootloader.
//!
//! Base on AN3155

use core::marker::PhantomData;

#[macro_use]
extern crate log;

#[macro_use(block)]
extern crate nb;

extern crate futures;

extern crate embedded_hal;
use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::serial::{Read, Write};

#[cfg(Feature = "structopt")]
extern crate structopt;

#[cfg(feature = "linux")]
extern crate linux_embedded_hal;

#[cfg(feature = "linux")]
pub mod linux;

pub const UART_DISC: u8 = 0x7F;

pub const UART_ACK: u8 = 0x79;
pub const UART_NACK: u8 = 0x1F;

pub trait SerialPort<E>: Write<u8, Error = E> + Read<u8, Error = E> {
    fn set_rts(&mut self, level: bool) -> Result<(), E>;
    fn set_dtr(&mut self, level: bool) -> Result<(), E>;
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum State {
    Init,
    Discovery,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Error<SerialError> {
    Serial(SerialError),
    Nack,
    NoAck,
    Timeout,
    InvalidResponse,
    BufferLength,
    Io(std::io::ErrorKind),
}

impl<SerialError> From<SerialError> for Error<SerialError> {
    fn from(e: SerialError) -> Self {
        Self::Serial(e)
    }
}

#[derive(Clone, PartialEq, Debug)]
#[cfg_attr(feature = "structopt", derive(structopt::StructOpt))]
pub struct Options {
    /// Do not reset the device on connection
    #[cfg_attr(feature = "structopt", structopt(long))]
    pub no_reset: bool,

    /// Timeout to wait for bootloader responses
    #[cfg_attr(feature = "structopt", structopt(long, default_value = "100"))]
    pub response_timeout_ms: u32,

    /// Period to poll for bootloader responses
    #[cfg_attr(feature = "structopt", structopt(long, default_value = "10"))]
    pub poll_delay_ms: u32,

    /// Period to wait for bootloader init before sending init character
    #[cfg_attr(feature = "structopt", structopt(long, default_value = "100"))]
    pub init_delay_ms: u32,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    /// Fetch bootloader version and allowed commands
    Get = 0x00,

    /// Gets the bootloader version and the Read Protection status of the Flash memory.
    GetVersionReadStatus = 0x01,
    
    /// Gets the chip ID
    GetId = 0x02,

    /// Reads up to 256 bytes of memory starting from an address specified by the application.
    ReadMemory = 0x11,

    /// Jumps to user application code located in the internal Flash memory or in the SRAM.
    Go = 0x21,

    /// Writes up to 256 bytes to the RAM or Flash memory starting from an address specified by the application.
    WriteMemory = 0x31,

    /// Erases from one to all the Flash memory pages.
    Erase = 0x43,

    /// Erases from one to all the Flash memory pages using two byte addressing mode (available only for v3.0 USART bootloader versions and above).
    ExtendedErase = 0x44,

    /// Enables the write protection for some sectors.
    WriteProtect = 0x63,

    /// Disables the write protection for all Flash memory sectors
    WriteUnprotect = 0x73,

    /// Enables the read protection
    ReadoutProtect = 0x82,

    /// Disables the read protection.
    ReadoutUnprotect = 0x92,
}

pub struct Programmer<P, D, E> {
    state: State,
    options: Options,
    port: P,
    delay: D,
    _err: PhantomData<E>,
}

impl<P, D, E> Programmer<P, D, E>
where
    P: SerialPort<E>,
    D: DelayMs<u32>,
    E: core::fmt::Debug,
{
    /// Create a new programmer instance
    pub fn new(port: P, delay: D, options: Options) -> Self {
        Self {
            state: State::Init,
            options,
            port,
            delay,
            _err: PhantomData,
        }
    }

    /// Fetch the programmer state
    pub fn state(&mut self) -> State {
        self.state
    }

    /// Execute the programmer
    pub fn run(&mut self) -> Result<(), Error<E>> {
        // Initialise bootloading
        self.init()
    }

    pub fn init(&mut self) -> Result<(), Error<E>> {
        // First, reset device
        debug!("Resetting device");

        self.reset(true)?;

        debug!("Sending discovery character");

        // Then, send discovery character
        self.port.write(UART_DISC).unwrap();

        // Wait for a response
        debug!("Awaiting bootloader response");
        let _ = self.await_ack();

        // Wait for bootloader to think a little
        self.delay.delay_ms(100);

        // Read info
        debug!("Reading bootloader info");
        let version = self.info()?;
        debug!("Bootloader version: 0x{:02x}", version);

        //debug!("Reading chip ID");
        //let id = self.chip_id()?;
        //debug!("ID: 0x{:04x}", id);

        self.delay.delay_ms(100);

        let mut data = [0u8; 10];
        self.read_mem(0x08000000, &mut data)?;
        debug!("Memory: {:02x?}", data);

        self.delay.delay_ms(100);

        // Return ok
        Ok(())
    }

    pub fn info(&mut self) -> Result<u8, Error<E>> {
        let mut data = [0u8; 12];

        // Write command
        self.write_cmd(Command::Get)?;

        // Await ack
        self.await_ack()?;

        // Read comand length
        let n = self.read_char()? as usize;

        debug!("Reading {} bytes", n);

        if data.len() < n {
            error!("RX buffer too short");
            return Err(Error::BufferLength);
        }

        // Read data
        for i in 0..n {
            data[i] = self.read_char()?;
        }

        debug!("Received: {:02x?}", &data[..n]);

        Ok(data[0])
    }

    /// Read memory block-by-block
    pub fn read_mem(&mut self, addr: u32, data: &mut [u8]) -> Result<(), Error<E>> {
        let mut index = 0;

        while index < data.len() {
            let n = (data.len() - index).min(256);

            self.read_mem_block(addr + index as u32, &mut data[index..])?;

            index += n;
        }

        Ok(())
    }

    fn read_mem_block(&mut self, addr: u32, data: &mut [u8]) -> Result<(), Error<E>> {

        assert!(data.len() <= 256, "blocks must be less than 256 bytes");

        // Write read command and await ack
        self.write_cmd(Command::ReadMemory)?;
        self.await_ack()?;
        
        // Write start address and await ack
        let addr= [(addr >> 24) as u8, (addr >> 16) as u8, (addr >> 8) as u8, addr as u8];
        self.write_csum(&addr)?;
        self.await_ack()?;
        
        let len =  [data.len() as u8];
        self.write_csum(&len)?;
        self.await_ack()?;

        for i in 0..data.len() {
            data[i] = self.read_char()?;
        }

        Ok(())
    }

    /// Reset the device using RTS while asserting DTR entering the bootloading or application
    pub fn reset(&mut self, bootloader: bool) -> Result<(), Error<E>> {
        // Assert RTS to reset the device
        self.port.set_rts(true)?;

        // Wait a moment for the device to turn off
        self.delay.delay_ms(10u32);

        if bootloader {
            // DTR signals to use bootloader
            self.port.set_dtr(true)?;
        }

        // RTS re-enables device
        self.port.set_rts(false)?;

        // Wait for bootloader or app to start
        self.delay.delay_ms(self.options.init_delay_ms);

        if bootloader {
            // De-assert DTR
            self.port.set_dtr(false)?;
        }

        Ok(())
    }

    /// Fetch device chip ID (not-working)
    pub fn chip_id(&mut self) -> Result<u16, Error<E>> {
        // Write GetID command
        self.write_cmd(Command::GetId)?;
        
        // Await ACK
        self.await_ack()?;

        // Read N (static sized)
        let _n = self.read_char()?;

        let b1 = self.read_char()?;
        let b2 = self.read_char()?;

        Ok((b1 as u16) << 8 | b2 as u16)
    }

    /// Write a bootloader command to the device
    pub fn write_cmd(&mut self, command: Command) -> Result<(), Error<E>> {
        // Write command
        let c1 = command.clone() as u8;
        let c2 = !c1;

        debug!("Writing command {:?} [0x{:02x}, 0x{:02x}]", command, c1, c2);

        block!(self.port.write(c1))?;
        block!(self.port.write(c2))?;

        Ok(())
    }

    /// Write data with xor checksum
    pub fn write_csum(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        let mut csum = 0x00;

        info!("Writing data with checksum: {:02x?}", data);

        for d in data {
            csum ^= *d;
            block!(self.port.write(*d))?;
        }

        block!(self.port.write(csum))?;

        Ok(())
    }

    /// Read a single character from the device
    pub fn read_char(&mut self) -> Result<u8, Error<E>> {
        let mut t = 0;

        loop {
            // Attempt to read from serial port
            match self.port.read() {
                Err(nb::Error::WouldBlock) => (),
                Err(nb::Error::Other(e)) => return Err(e.into()),
                Ok(v) => return Ok(v)
            };

            // Wait for delay period
            self.delay.delay_ms(self.options.poll_delay_ms);
            t += self.options.poll_delay_ms;

            if t > self.options.response_timeout_ms {
                error!("Receive timeout");
                return Err(Error::Timeout);
            }
        }
    }

    /// Await an ack from the bootloader
    fn await_ack(&mut self) -> Result<(), Error<E>> {
        let v = self.read_char()?;
        match v {
            UART_ACK => {
                debug!("Received ACK!");
                Ok(())
            },
            UART_NACK => {
                debug!("Received NACK?!@");
                Err(Error::Nack)
            },
            _ => {
                error!("Unexpected response: 0x{:02x}", v);
                Err(Error::InvalidResponse)
            }
        }
    }
}
