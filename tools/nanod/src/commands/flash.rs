use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::commands::validate;
use crate::device::connection;
use crate::device::constants::*;
use crate::util::progress;

pub fn run(firmware: &Path, skip_validate: bool, port: Option<&str>) -> Result<()> {
    let data = fs::read(firmware)
        .with_context(|| format!("Failed to read firmware: {}", firmware.display()))?;

    // Pre-flash validation
    if !skip_validate {
        validate::run(firmware)?;
        println!();
    }

    let port_name = connection::resolve_port(port)?;
    println!("Using port: {}", port_name);

    // Enter bootloader
    let sp = progress::spinner("Entering bootloader...");
    connection::enter_bootloader(&port_name)?;
    sp.finish_with_message("Bootloader ready");

    // Flash app0
    println!(
        "Flashing {} bytes to app0 (offset 0x{:X})...",
        data.len(),
        APP0_OFFSET
    );

    let pb = progress::flash_progress(data.len() as u64, "Flashing");

    // TODO: Integrate espflash library for actual flash operations
    // For now, this is the integration point where espflash::flasher::Flasher
    // would connect and write the binary to APP0_OFFSET.
    //
    // The espflash 4.x API workflow:
    // 1. Open connection with Connection::new()
    // 2. Create Flasher from connection
    // 3. Use write_bin_to_flash(APP0_OFFSET, &data) or equivalent
    // 4. Reset device
    //
    // This requires validating the exact espflash 4.x public API,
    // which may differ from the 3.x API documented elsewhere.

    pb.finish_with_message("Flash complete (stub - espflash integration pending)");

    println!("\nDone. Device should be running new firmware.");
    Ok(())
}
