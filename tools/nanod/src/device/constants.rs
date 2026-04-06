/// Total flash size in bytes (4 MB)
pub const FLASH_SIZE: u32 = 4 * 1024 * 1024;

/// App0 (ota_0) partition offset
pub const APP0_OFFSET: u32 = 0x10000;

/// App0 (ota_0) partition size (1.25 MB)
pub const APP0_SIZE: u32 = 0x140000;

/// OTA data partition offset
pub const OTADATA_OFFSET: u32 = 0xe000;

/// OTA data partition size
pub const OTADATA_SIZE: u32 = 0x2000;

/// Upload baud rate for flashing
pub const UPLOAD_BAUD: u32 = 460800;

/// Monitor baud rate for serial debug
pub const MONITOR_BAUD: u32 = 115200;

/// USB Vendor ID (Adafruit)
pub const USB_VID: u16 = 0x239A;

/// USB Product IDs for NanoD
pub const USB_PIDS: &[u16] = &[0x811B, 0x011B, 0x811C];

/// ESP32 image header magic byte
pub const ESP_IMAGE_MAGIC: u8 = 0xE9;

/// ESP32-S3 chip ID
pub const ESP32_S3_CHIP_ID: u16 = 0x0009;
