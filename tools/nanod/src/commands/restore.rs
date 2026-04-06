use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use espflash::connection::ResetBeforeOperation;

use crate::device::connection;
use crate::device::constants::*;
use crate::util::progress::FlashProgress;

pub fn run(image: &Path, force: bool, port: Option<&str>) -> Result<()> {
    if !force {
        bail!(
            "Restore overwrites the ENTIRE flash including bootloader, NVS, and SPIFFS.\n\
             Pass --force to confirm this destructive operation."
        );
    }

    let data =
        fs::read(image).with_context(|| format!("Failed to read image: {}", image.display()))?;

    if data.len() != FLASH_SIZE as usize {
        println!(
            "WARNING: Image size ({} bytes) does not match flash size ({} bytes)",
            data.len(),
            FLASH_SIZE
        );
        println!("Proceeding anyway due to --force flag.");
    }

    println!("Restoring {} bytes to flash...", data.len());

    let mut flasher = connection::connect_flasher(port, ResetBeforeOperation::default())?;

    let mut progress = FlashProgress::new("Restoring");
    flasher
        .write_bin_to_flash(0, &data, &mut progress)
        .context("Failed to write flash image")?;

    flasher
        .connection()
        .reset()
        .context("Failed to reset device")?;

    println!("\nRestore complete.");
    Ok(())
}
