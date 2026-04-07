use std::path::Path;

use anyhow::{Context, Result};
use espflash::connection::ResetBeforeOperation;

use crate::device::connection;
use crate::device::constants::*;

pub fn run(output: &Path, port: Option<&str>) -> Result<()> {
    println!("Backing up {} bytes from flash...", FLASH_SIZE);

    let mut flasher = connection::connect_flasher(port, ResetBeforeOperation::default())?;

    let block_size = 0x1000_u32; // 4KB sectors
    let max_in_flight = 64_u32;

    flasher
        .read_flash(
            0,
            FLASH_SIZE,
            block_size,
            max_in_flight,
            output.to_path_buf(),
        )
        .context("Failed to read flash")?;

    // Reset device after backup
    flasher
        .connection()
        .reset()
        .context("Failed to reset device")?;

    println!("Backup saved to: {}", output.display());
    Ok(())
}
