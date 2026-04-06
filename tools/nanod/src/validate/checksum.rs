use crate::util::error::NanodError;

/// Verify the ESP32 image checksum.
///
/// ESP32 image format: after the 8-byte header, there are N segments.
/// Each segment has an 8-byte header (offset + size) followed by `size` bytes of data.
/// The checksum is XOR of all segment data bytes, seeded with 0xEF.
/// The checksum byte is at the end of the image (padded to 16-byte alignment).
pub fn verify_checksum(data: &[u8]) -> Result<(), NanodError> {
    if data.len() < 24 {
        return Err(NanodError::InvalidImage {
            reason: "firmware too short for checksum verification".into(),
        });
    }

    let segment_count = data[1] as usize;
    let mut offset: usize = 24; // ESP32 extended image header is 24 bytes
    let mut checksum: u8 = 0xEF;

    for seg in 0..segment_count {
        if offset + 8 > data.len() {
            return Err(NanodError::InvalidImage {
                reason: format!("segment {} header truncated at offset {}", seg, offset),
            });
        }

        let seg_size =
            u32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]])
                as usize;
        offset += 8; // skip segment header

        if offset + seg_size > data.len() {
            return Err(NanodError::InvalidImage {
                reason: format!(
                    "segment {} data truncated: need {} bytes at offset {}, have {}",
                    seg,
                    seg_size,
                    offset,
                    data.len() - offset
                ),
            });
        }

        for &byte in &data[offset..offset + seg_size] {
            checksum ^= byte;
        }
        offset += seg_size;
    }

    // Checksum byte is at the end, padded to 16-byte boundary
    // The byte just before the 16-byte aligned end is the checksum
    let padded_end = (offset + 16) & !15;
    if padded_end > data.len() {
        // If file is shorter than expected padding, check at the actual end
        let expected = data[data.len() - 1];
        if checksum != expected {
            return Err(NanodError::ChecksumMismatch {
                expected,
                actual: checksum,
            });
        }
    } else {
        // Checksum is at padded_end - 1
        let expected = data[padded_end - 1];
        if checksum != expected {
            return Err(NanodError::ChecksumMismatch {
                expected,
                actual: checksum,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_too_short_rejected() {
        assert!(verify_checksum(&[0xE9; 10]).is_err());
    }

    #[test]
    fn test_zero_segments_valid() {
        // 24-byte header with 0 segments, checksum at padded position
        let mut data = vec![0u8; 32]; // 32 = next 16-byte boundary after 24
        data[0] = 0xE9; // magic
        data[1] = 0; // 0 segments
        // Checksum of no data XOR'd = 0xEF, placed at byte 31 (padded_end - 1)
        data[31] = 0xEF;
        assert!(verify_checksum(&data).is_ok());
    }
}
