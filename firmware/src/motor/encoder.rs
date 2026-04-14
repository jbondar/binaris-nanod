use core::f32::consts::PI;

use esp_idf_hal::gpio::AnyOutputPin;
use esp_idf_hal::spi::{self, SpiDeviceDriver, SpiConfig};
use esp_idf_hal::units::Hertz;
use esp_idf_sys::EspError;

const TWO_PI: f32 = 2.0 * PI;
const MT6701_RESOLUTION: f32 = 16384.0; // 14-bit

/// MT6701 magnetic angle sensor via SSI (SPI mode 3).
pub struct Mt6701Encoder<'a> {
    spi: SpiDeviceDriver<'a, &'a spi::SpiDriver<'a>>,
    prev_raw: u16,
    full_rotations: i32,
    angle_offset: f32,
}

impl<'a> Mt6701Encoder<'a> {
    /// Create a new MT6701 encoder on the given SPI bus.
    pub fn new(
        spi_driver: &'a spi::SpiDriver<'a>,
        cs_pin: AnyOutputPin<'a>,
    ) -> Result<Self, EspError> {
        let config = SpiConfig::new()
            .baudrate(Hertz(1_000_000))
            .data_mode(embedded_hal::spi::MODE_3);

        let spi = SpiDeviceDriver::new(spi_driver, Some(cs_pin), &config)?;

        Ok(Self {
            spi,
            prev_raw: 0,
            full_rotations: 0,
            angle_offset: 0.0,
        })
    }

    /// Read the current angle in radians (continuous, multi-turn).
    pub fn read_angle(&mut self) -> Result<f32, EspError> {
        let raw = self.read_raw()?;

        // Track full rotations
        let diff = raw as i32 - self.prev_raw as i32;
        if diff < -(MT6701_RESOLUTION as i32 / 2) {
            self.full_rotations += 1;
        } else if diff > (MT6701_RESOLUTION as i32 / 2) {
            self.full_rotations -= 1;
        }
        self.prev_raw = raw;

        let angle = (self.full_rotations as f32 * TWO_PI)
            + (raw as f32 / MT6701_RESOLUTION * TWO_PI)
            - self.angle_offset;

        Ok(angle)
    }

    /// Read raw 14-bit angle value.
    fn read_raw(&mut self) -> Result<u16, EspError> {
        use embedded_hal::spi::SpiDevice;
        let mut buf = [0xFFu8; 3]; // Send 0xFF to clock data in (24 bits for MT6701)
        self.spi
            .transfer_in_place(&mut buf)
            .map_err(|_| EspError::from_infallible::<{ esp_idf_sys::ESP_FAIL }>())?;

        // MT6701 SSI: first 14 bits are the angle value (MSB first)
        let raw = ((buf[0] as u16) << 6) | ((buf[1] as u16) >> 2);

        Ok(raw & 0x3FFF)
    }

    /// Set the current position as the zero reference.
    pub fn set_zero(&mut self) -> Result<(), EspError> {
        let raw = self.read_raw()?;
        self.angle_offset = raw as f32 / MT6701_RESOLUTION * TWO_PI;
        self.full_rotations = 0;
        self.prev_raw = raw;
        Ok(())
    }

    /// Get the raw angle for calibration purposes.
    pub fn get_raw_angle(&mut self) -> Result<f32, EspError> {
        let raw = self.read_raw()?;
        Ok(raw as f32 / MT6701_RESOLUTION * TWO_PI)
    }
}
