

pub const UART_DISC: u8 = 0x7F;

pub const UART_ACK: u8 = 0x79;
pub const UART_NACK: u8 = 0x1F;

pub const MAX_CHUNK: usize = 256;

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
