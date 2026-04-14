use esp_idf_svc::nvs::{EspNvs, EspNvsPartition, NvsDefault};
use esp_idf_sys::EspError;

use crate::haptic::profile::{Direction, MotorCalibration, NOT_SET};

const NVS_NAMESPACE: &str = "nano_D";
const KEY_DIRECTION: &str = "direction";
const KEY_ZERO_ANGLE: &str = "zero_angle";

/// Load motor calibration from NVS.
/// Opens read-write to auto-create namespace if it doesn't exist (fresh flash).
pub fn load_calibration(
    nvs_partition: EspNvsPartition<NvsDefault>,
) -> Result<MotorCalibration, EspError> {
    let nvs = EspNvs::new(nvs_partition, NVS_NAMESPACE, true)?;

    let direction = match nvs.get_u8(KEY_DIRECTION)? {
        Some(1) => Direction::Cw,
        Some(2) => Direction::Ccw,
        _ => Direction::Unknown,
    };

    let zero_angle = get_f32_from_nvs(&nvs, KEY_ZERO_ANGLE).unwrap_or(NOT_SET);

    Ok(MotorCalibration {
        direction,
        zero_angle,
    })
}

/// Store motor calibration to NVS.
pub fn store_calibration(
    nvs_partition: EspNvsPartition<NvsDefault>,
    cal: &MotorCalibration,
) -> Result<(), EspError> {
    let mut nvs = EspNvs::new(nvs_partition, NVS_NAMESPACE, true)?;

    let dir_val: u8 = match cal.direction {
        Direction::Unknown => 0,
        Direction::Cw => 1,
        Direction::Ccw => 2,
    };
    nvs.set_u8(KEY_DIRECTION, dir_val)?;
    set_f32_in_nvs(&mut nvs, KEY_ZERO_ANGLE, cal.zero_angle)?;

    Ok(())
}

/// Read an f32 from NVS by storing as raw u32 bits.
fn get_f32_from_nvs(nvs: &EspNvs<NvsDefault>, key: &str) -> Result<f32, EspError> {
    match nvs.get_u32(key)? {
        Some(bits) => Ok(f32::from_bits(bits)),
        None => Ok(NOT_SET),
    }
}

/// Write an f32 to NVS as raw u32 bits.
fn set_f32_in_nvs(nvs: &mut EspNvs<NvsDefault>, key: &str, val: f32) -> Result<(), EspError> {
    nvs.set_u32(key, val.to_bits())
}
