use crate::device::constants::*;
use crate::util::error::NanodError;

/// Validate firmware binary size fits in the app0 partition.
pub fn check_size(data: &[u8]) -> Result<(), NanodError> {
    let max = APP0_SIZE as u64;
    let size = data.len() as u64;
    if size > max {
        return Err(NanodError::FirmwareTooLarge { size, max });
    }
    if size == 0 {
        return Err(NanodError::InvalidImage {
            reason: "firmware file is empty".into(),
        });
    }
    Ok(())
}

/// Validate the ESP32 image header magic byte.
pub fn check_magic(data: &[u8]) -> Result<(), NanodError> {
    if data.is_empty() {
        return Err(NanodError::InvalidImage {
            reason: "firmware file is empty".into(),
        });
    }
    if data[0] != ESP_IMAGE_MAGIC {
        return Err(NanodError::InvalidImage {
            reason: format!(
                "bad magic byte: expected 0x{:02X}, got 0x{:02X}",
                ESP_IMAGE_MAGIC, data[0]
            ),
        });
    }
    Ok(())
}

/// Validate the chip ID matches ESP32-S3.
/// The chip ID is stored as a little-endian u16 at offset 12 in the image header.
pub fn check_chip_id(data: &[u8]) -> Result<(), NanodError> {
    if data.len() < 14 {
        return Err(NanodError::InvalidImage {
            reason: "firmware too short to contain image header".into(),
        });
    }
    let chip_id = u16::from_le_bytes([data[12], data[13]]);
    if chip_id != ESP32_S3_CHIP_ID {
        return Err(NanodError::WrongChip {
            expected: ESP32_S3_CHIP_ID,
            actual: chip_id,
        });
    }
    Ok(())
}

/// Run all binary validation checks.
pub fn validate_all(data: &[u8]) -> Result<(), NanodError> {
    check_size(data)?;
    check_magic(data)?;
    check_chip_id(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_firmware_rejected() {
        assert!(check_size(&[]).is_err());
        assert!(check_magic(&[]).is_err());
    }

    #[test]
    fn test_wrong_magic_rejected() {
        let data = vec![0x00; 24];
        assert!(check_magic(&data).is_err());
    }

    #[test]
    fn test_correct_magic_accepted() {
        let mut data = vec![0x00; 24];
        data[0] = ESP_IMAGE_MAGIC;
        assert!(check_magic(&data).is_ok());
    }

    #[test]
    fn test_wrong_chip_id_rejected() {
        let mut data = vec![0x00; 24];
        data[0] = ESP_IMAGE_MAGIC;
        // Set chip ID to ESP32 (0x0000) instead of ESP32-S3 (0x0009)
        data[12] = 0x00;
        data[13] = 0x00;
        assert!(check_chip_id(&data).is_err());
    }

    #[test]
    fn test_correct_chip_id_accepted() {
        let mut data = vec![0x00; 24];
        data[0] = ESP_IMAGE_MAGIC;
        data[12] = 0x09;
        data[13] = 0x00;
        assert!(check_chip_id(&data).is_ok());
    }

    #[test]
    fn test_firmware_too_large_rejected() {
        let data = vec![0x00; APP0_SIZE as usize + 1];
        assert!(check_size(&data).is_err());
    }

    #[test]
    fn test_firmware_at_max_size_accepted() {
        let data = vec![0x00; APP0_SIZE as usize];
        assert!(check_size(&data).is_ok());
    }
}
