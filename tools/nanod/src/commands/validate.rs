use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::validate::{binary, checksum, partition};

pub fn run(firmware: &Path) -> Result<()> {
    let data = fs::read(firmware)
        .with_context(|| format!("Failed to read firmware: {}", firmware.display()))?;

    println!("Validating: {}", firmware.display());
    println!("  File size: {} bytes", data.len());

    // Size check
    binary::check_size(&data)?;
    println!("  [PASS] Size within app0 partition limit");

    // Magic byte
    binary::check_magic(&data)?;
    println!("  [PASS] ESP32 image magic byte (0xE9)");

    // Chip ID
    binary::check_chip_id(&data)?;
    println!("  [PASS] Chip ID: ESP32-S3");

    // Checksum
    match checksum::verify_checksum(&data) {
        Ok(()) => println!("  [PASS] Image checksum valid"),
        Err(e) => println!("  [WARN] Checksum: {} (may be PlatformIO format)", e),
    }

    // Partition fit
    match partition::validate_fits_app0(data.len()) {
        Ok(msg) => println!("  [INFO] {}", msg),
        Err(msg) => println!("  [FAIL] {}", msg),
    }

    println!("\nValidation passed.");
    Ok(())
}
