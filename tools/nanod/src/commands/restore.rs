use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::device::connection;
use crate::device::constants::*;
use crate::util::progress;

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

    let port_name = connection::resolve_port(port)?;
    println!("Using port: {}", port_name);

    let sp = progress::spinner("Entering bootloader...");
    connection::enter_bootloader(&port_name)?;
    sp.finish_with_message("Bootloader ready");

    println!("Writing {} bytes to flash...", data.len());
    let _pb = progress::flash_progress(data.len() as u64, "Restoring");

    // TODO: Integrate espflash library
    // Write full image starting at offset 0

    println!("Restore complete (stub - espflash integration pending)");
    Ok(())
}
