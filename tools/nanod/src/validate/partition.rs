use crate::device::partitions::PARTITIONS;

/// Validate that a firmware binary fits in the app0 partition.
/// Returns the partition info string for display.
pub fn validate_fits_app0(firmware_size: usize) -> Result<String, String> {
    let app0 = PARTITIONS
        .iter()
        .find(|p| p.name == "app0")
        .expect("app0 partition not found in table");

    if firmware_size > app0.size as usize {
        return Err(format!(
            "Firmware ({} bytes) exceeds app0 partition ({} bytes, {:.1}% over)",
            firmware_size,
            app0.size,
            ((firmware_size as f64 / app0.size as f64) - 1.0) * 100.0
        ));
    }

    let usage_pct = (firmware_size as f64 / app0.size as f64) * 100.0;
    Ok(format!(
        "Firmware fits app0: {} / {} bytes ({:.1}% used)",
        firmware_size, app0.size, usage_pct
    ))
}
