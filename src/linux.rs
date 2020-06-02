
use std::io::{ErrorKind as IoErrorKind};
use std::path::Path;

use linux_embedded_hal::{Delay, Serial};
use linux_embedded_hal::serial_core::{SerialDevice as _, SerialPortSettings as _, Error as SerialError, BaudRate, FlowControl, Parity};

use crate::{SerialPort, Programmer, Options};

impl SerialPort<std::io::ErrorKind> for Serial {
    fn set_rts(&mut self, level: bool) -> Result<(), std::io::ErrorKind> {
        self.0.set_rts(level).unwrap();
        Ok(())
    }
    fn set_dtr(&mut self, level: bool) -> Result<(), std::io::ErrorKind> {
        self.0.set_dtr(level).unwrap();
        Ok(())
    }
}

impl Programmer<Serial, Delay, IoErrorKind>
{
    /// Create a new linux serial port programmer instance
    pub fn linux<P: AsRef<Path>>(port: P, baud: usize, options: Options) -> Result<Self, SerialError> {
        // Open port
        let mut port = Serial::open(port.as_ref())?;

        // Apply settings
        let mut settings = port.0.read_settings()?;
        settings.set_baud_rate(BaudRate::from_speed(baud))?;
        settings.set_flow_control(FlowControl::FlowNone);
        settings.set_parity(Parity::ParityEven);
        port.0.write_settings(&settings)?;

        // Return instance
        Ok(Self::new(port, Delay{}, options))
    }
}