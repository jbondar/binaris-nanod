use std::path::Path;

use anyhow::Result;

use crate::device::connection;
use crate::device::constants::*;
use crate::util::progress;

pub fn run(output: &Path, port: Option<&str>) -> Result<()> {
    let port_name = connection::resolve_port(port)?;
    println!("Using port: {}", port_name);

    let sp = progress::spinner("Entering bootloader...");
    connection::enter_bootloader(&port_name)?;
    sp.finish_with_message("Bootloader ready");

    println!("Reading {} bytes from flash...", FLASH_SIZE);
    let _pb = progress::flash_progress(FLASH_SIZE as u64, "Reading");

    // TODO: Integrate espflash library for flash readback
    // espflash::flasher::Flasher::read_flash(0, FLASH_SIZE)
    // then write result to output file

    println!(
        "Backup saved to: {} (stub - espflash integration pending)",
        output.display()
    );
    Ok(())
}
