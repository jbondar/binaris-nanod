use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use espflash::connection::ResetBeforeOperation;

use crate::commands::validate;
use crate::device::connection;
use crate::device::constants::*;
use crate::util::progress::FlashProgress;

pub fn run(firmware: &Path, skip_validate: bool, port: Option<&str>) -> Result<()> {
    let data = fs::read(firmware)
        .with_context(|| format!("Failed to read firmware: {}", firmware.display()))?;

    // Pre-flash validation
    if !skip_validate {
        validate::run(firmware)?;
        println!();
    }

    println!(
        "Flashing {} bytes to app0 (offset 0x{:X})...",
        data.len(),
        APP0_OFFSET
    );

    let mut flasher = connection::connect_flasher(port, ResetBeforeOperation::default())?;

    let mut progress = FlashProgress::new("Flashing");
    flasher
        .write_bin_to_flash(APP0_OFFSET, &data, &mut progress)
        .context("Failed to write firmware to flash")?;

    // Reset device to run new firmware
    flasher
        .connection()
        .reset()
        .context("Failed to reset device")?;

    println!("\nDone. Device should be running new firmware.");
    Ok(())
}
