
#[macro_use]
extern crate log;

extern crate structopt;
use structopt::StructOpt;

extern crate simplelog;
use simplelog::{SimpleLogger, LevelFilter, Config};

use stm32_uart_loader::{Programmer, Options};

#[derive(Clone, Debug, StructOpt)]
pub struct Args {
    /// Serial port to connect to
    #[structopt(long, default_value="/dev/ttyUSB0")]
    port: String,

    /// Serial port baud rate
    #[structopt(long, default_value="57600")]
    baud: usize,

    #[structopt(flatten)]
    options: Options,

    /// Log level for console output
    #[structopt(long, default_value="debug")]
    log_level: LevelFilter,
}



fn main() {
    // Parse out arguments
    let o = Args::from_args();

    // Configure logger
    let _ = SimpleLogger::init(o.log_level, Config::default());

    info!("Connecting to serial port");

    let mut p = match Programmer::linux(&o.port, o.baud, o.options) {
        Ok(p) => p,
        Err(e) => {
            println!("Error connecting to serial port: {:?}", e);
            return;
        }
    };

    info!("Connecting to bootloader");

    if let Err(e) = p.init() {
        error!("Error connecting to bootloader: {:?}", e);
        return
    }

    info!("Bootloader connected!");

}


