use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use espflash::connection::ResetBeforeOperation;

use crate::device::connection;
use crate::device::constants::*;
use crate::util::progress::FlashProgress;
use crate::validate::binary;

pub fn run(firmware: &Path, port: Option<&str>) -> Result<()> {
    let data = fs::read(firmware)
        .with_context(|| format!("Failed to read firmware: {}", firmware.display()))?;

    // Still validate the binary even in recovery mode
    binary::validate_all(&data)?;
    println!("Firmware validated: {} bytes", data.len());

    // In recovery mode, device is already in ROM download mode (BOOT + reset).
    // Use default reset strategy — espflash will handle the connection.
    // If the device is already in bootloader, it will connect directly.
    println!("Connecting in recovery mode...");

    let mut flasher = connection::connect_flasher(port, ResetBeforeOperation::default())?;

    println!(
        "Flashing {} bytes to app0 (offset 0x{:X})...",
        data.len(),
        APP0_OFFSET
    );

    let mut progress = FlashProgress::new("Recovering");
    flasher
        .write_bin_to_flash(APP0_OFFSET, &data, &mut progress)
        .context("Failed to write firmware")?;

    flasher
        .connection()
        .reset()
        .context("Failed to reset device")?;

    println!("\nRecovery complete. Device should be running firmware.");
    Ok(())
}
