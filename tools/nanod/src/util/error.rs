use thiserror::Error;

#[derive(Error, Debug)]
pub enum NanodError {
    #[error("Device not found. Is the NanoD connected?")]
    DeviceNotFound,

    #[error("Firmware too large: {size} bytes (max {max} bytes)")]
    FirmwareTooLarge { size: u64, max: u64 },

    #[error("Invalid ESP32 image: {reason}")]
    InvalidImage { reason: String },

    #[error("Checksum mismatch: expected 0x{expected:02x}, got 0x{actual:02x}")]
    ChecksumMismatch { expected: u8, actual: u8 },

    #[error("Wrong chip ID: expected ESP32-S3 (0x{expected:04x}), got 0x{actual:04x}")]
    WrongChip { expected: u16, actual: u16 },

    #[error("Serial port error: {0}")]
    SerialError(#[from] serialport::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
