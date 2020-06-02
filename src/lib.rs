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
use embedded_hal::serial::{Write, Read};
use embedded_hal::blocking::delay::DelayMs;

#[cfg(Feature = "structopt")]
extern crate structopt;

#[cfg(feature = "linux")]
extern crate linux_embedded_hal;

#[cfg(feature = "linux")]
pub mod linux;

pub const UART_DISC: u8 = 0x7F;

pub const UART_ACK: u8 = 0x79;
pub const UART_NACK: u8 = 0x1F;

pub trait SerialPort<E>: Write<u8, Error=E> + Read<u8, Error=E> {
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
    ResponseTimeout,
    InvalidResponse,
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
    #[cfg_attr(feature = "structopt", structopt(long, default_value="10"))]
    pub response_timeout_ms: u32,

    /// Period to poll for bootloader responses
    #[cfg_attr(feature = "structopt", structopt(long, default_value="1"))]
    pub poll_delay_ms: u32,

    /// Period to wait for bootloader init before sending init character
    #[cfg_attr(feature = "structopt", structopt(long, default_value="1"))]
    pub init_delay_ms: u32,
}

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



impl <P, D, E> Programmer<P, D, E>
where 
    P: SerialPort<E>,
    D: DelayMs<u32>,
    E: core::fmt::Debug,
{
    /// Create a new programmer instance
    pub fn new(port: P, delay: D, options: Options) -> Self {
        Self{state: State::Init, options, port, delay, _err: PhantomData}
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
        if !self.options.no_reset {
            debug!("Resetting device");

            self.port.set_dtr(true)?;
            self.port.set_rts(true)?;

            self.delay.delay_ms(100u32);

            self.port.set_dtr(false)?;
            self.port.set_rts(false)?;

            self.delay.delay_ms(self.options.init_delay_ms);
        }

        debug!("Sending discovery character");

        // Then, send discovery character
        self.port.write(UART_DISC).unwrap();

        // Wait for a response
        debug!("Awaiting bootloader response");
        self.await_ack()?;

        // Return ok
        Ok(())
    }

    pub fn read_cmd(&mut self, command: Command, data: &mut[u8]) 
    -> Result<usize, Error<E>> {

        // Write command
        let c = command as u8;
        block!(self.port.write(c))?;
        block!(self.port.write(c ^ 0x00))?;

        // Await ack
        self.await_ack()?;

        // Read 

        Ok(0)
    }

    fn await_ack(&mut self) -> Result<(), Error<E>> {
        let mut t = 0;

        loop {
            // Attempt to read from serial port
            match self.port.read() {
                Err(nb::Error::WouldBlock) => (),
                Err(nb::Error::Other(e)) => return Err(e.into()),
                Ok(v) if v == UART_ACK => {
                    debug!("Received bootloader ack");
                    return Ok(())
                },
                Ok(v) if v == UART_NACK => {
                    debug!("Received bootloader nack");
                    return Err(Error::Nack)
                },
                Ok(v) => {
                    debug!("Received unexpected value: 0x{:x}", v);
                    return Err(Error::InvalidResponse)
                }
            };

            // Wait for delay period
            self.delay.delay_ms(self.options.poll_delay_ms);
            t += self.options.poll_delay_ms;

            if t > self.options.response_timeout_ms {
                error!("Receive timeout");
                return Err(Error::ResponseTimeout)
            }
        }
    }

    pub fn read_data(&mut self, data: &mut[u8]) 
    -> Result<usize, Error<E>> {

        unimplemented!()
    }

}

