use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::device::connection;
use crate::device::constants::*;
use crate::util::progress;
use crate::validate::binary;

pub fn run(firmware: &Path, port: Option<&str>) -> Result<()> {
    let data = fs::read(firmware)
        .with_context(|| format!("Failed to read firmware: {}", firmware.display()))?;

    // Still validate the binary even in recovery mode
    binary::validate_all(&data)?;
    println!("Firmware validated: {} bytes", data.len());

    // In recovery mode, device is already in ROM download mode (BOOT + reset).
    // Skip 1200bps touch — connect directly.
    let port_name = connection::resolve_port(port)?;
    println!("Using port: {} (recovery mode — device should be in bootloader)", port_name);

    println!(
        "Flashing {} bytes to app0 (offset 0x{:X})...",
        data.len(),
        APP0_OFFSET
    );
    let _pb = progress::flash_progress(data.len() as u64, "Recovering");

    // TODO: Integrate espflash library
    // 1. Connect directly (no reset sequence)
    // 2. Write firmware to APP0_OFFSET
    // 3. Clear otadata to force boot from app0
    // 4. Reset device

    println!("Recovery complete (stub - espflash integration pending)");
    Ok(())
}
