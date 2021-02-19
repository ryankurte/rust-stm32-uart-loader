use std::{num::ParseIntError};

#[macro_use]
extern crate log;

use anyhow::Context;
use structopt::StructOpt;
use simplelog::{Config, LevelFilter, SimpleLogger};

use stm32_uart_loader::{Options, Programmer};

#[derive(Clone, Debug, StructOpt)]
pub struct Args {
    /// Serial port to connect to
    #[structopt(long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// Serial port baud rate
    #[structopt(long, default_value = "57600")]
    baud: usize,

    #[structopt(subcommand)]
    command: Commands,

    #[structopt(flatten)]
    options: Options,

    /// Log level for console output
    #[structopt(long, default_value = "info")]
    log_level: LevelFilter,
}


#[derive(Clone, Debug, StructOpt)]
pub enum Commands {
    Read {
        /// Offset from which to start memory read
        #[structopt(long, parse(try_from_str=u32_from_hex), default_value="0x08000000")]
        offset: u32,

        /// Length of memory to read
        #[structopt(long, parse(try_from_str=bytefmt::parse))]
        length: u64,

        /// File to read data into
        #[structopt(long)]
        file: String,
    },
    Write {
        /// Offset from which to start memory write
        #[structopt(long, parse(try_from_str=u32_from_hex), default_value="0x08000000")]
        offset: u32,

        /// File to read data from
        #[structopt(long)]
        file: String,
    },
    Erase {
        /// Offset from which to start memory read
        #[structopt(long, default_value="0")]
        page_offset: u8,

        /// Length of memory to read
        #[structopt(long)]
        page_count: u8,
    },
    EraseAll,
    //ChipId,
}

fn u32_from_hex(s: &str) -> Result<u32, ParseIntError> {
    let s = s.trim_start_matches("0x");
    u32::from_str_radix(s, 16)
}

fn main() -> Result<(), anyhow::Error> {
    // Parse out arguments
    let o = Args::from_args();

    // Configure logger
    let _ = SimpleLogger::init(o.log_level, Config::default());

    debug!("Connecting to bootloader");

    let mut p = Programmer::linux(&o.port, o.baud, o.options)
        .context("Error connecting to bootloader")?;

    // Execute commands
    match &o.command {
        Commands::Read{offset, length, file} => {
            info!("Reading {} bytes from memory at offset 0x{:08x}", length, offset);

            let mut data = vec![0u8; *length as usize];
            p.read(*offset, &mut data).context("Error reading memory")?;

            std::fs::write(file, data)
                .context("Failure writing to file")?;
        },

        Commands::Write{offset, file} => {
            let data = std::fs::read(file)
                .context("Failure reading from file")?;

            info!("Reading {} bytes from memory at offset 0x{:08x}", data.len(), offset);

            p.write(*offset, &data)
                .context("Error writing memory")?;
        },
        Commands::Erase{page_offset, page_count} => {
            info!("Erasing {} pages from index {}", page_count, page_offset);

            p.erase(*page_offset, *page_count)
                .context("Error erasing pages")?;
        },
        Commands::EraseAll => {
            info!("Erasing entire device flash");

            p.erase_all()
                .context("Error erasing pages")?;
        }
    }

    Ok(())
}
