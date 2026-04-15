use core::f32::consts::PI;

use esp_idf_sys::*;

const TWO_PI: f32 = 2.0 * PI;
const MT6701_RESOLUTION: f32 = 16384.0; // 14-bit

/// MT6701 magnetic angle sensor via SSI (SPI mode 3).
///
/// Uses low-level `spi_device_polling_transmit` for minimum latency
/// in the FOC control loop (vs the HAL's interrupt-based driver).
pub struct Mt6701Encoder {
    spi_device: spi_device_handle_t,
    prev_raw: u16,
    full_rotations: i32,
    angle_offset: f32,
}

impl Mt6701Encoder {
    /// Initialize SPI device for encoder. Uses polling mode for fast reads.
    pub fn new(
        host: spi_host_device_t,
        cs_pin: i32,
        sclk_pin: i32,
        miso_pin: i32,
        mosi_pin: i32,
    ) -> Result<Self, EspError> {
        // Configure SPI bus — use zeroed struct and set fields individually
        let mut bus_config: spi_bus_config_t = unsafe { core::mem::zeroed() };
        bus_config.__bindgen_anon_1.mosi_io_num = mosi_pin;
        bus_config.__bindgen_anon_2.miso_io_num = miso_pin;
        bus_config.sclk_io_num = sclk_pin;
        bus_config.__bindgen_anon_3.quadwp_io_num = -1;
        bus_config.__bindgen_anon_4.quadhd_io_num = -1;
        bus_config.max_transfer_sz = 32;
        bus_config.flags = SPICOMMON_BUSFLAG_MASTER;

        esp!(unsafe { spi_bus_initialize(host, &bus_config, 0) })?; // 0 = no DMA

        // Configure SPI device — polling mode, 10MHz, SPI mode 3
        let mut dev_config: spi_device_interface_config_t = unsafe { core::mem::zeroed() };
        dev_config.clock_speed_hz = 10_000_000;
        dev_config.mode = 3; // CPOL=1, CPHA=1
        dev_config.spics_io_num = cs_pin;
        dev_config.queue_size = 1;
        dev_config.flags = SPI_DEVICE_HALFDUPLEX;

        let mut spi_device: spi_device_handle_t = core::ptr::null_mut();
        esp!(unsafe { spi_bus_add_device(host, &dev_config, &mut spi_device) })?;

        Ok(Self {
            spi_device,
            prev_raw: 0,
            full_rotations: 0,
            angle_offset: 0.0,
        })
    }

    /// Read the current angle in radians (continuous, multi-turn).
    #[inline]
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

    /// Read raw 14-bit angle value using polling SPI (no ISR overhead).
    #[inline]
    fn read_raw(&mut self) -> Result<u16, EspError> {
        let mut rx_buf = [0u8; 4]; // 4-byte aligned buffer
        let mut trans: spi_transaction_t = unsafe { core::mem::zeroed() };
        trans.rxlength = 24; // Read 24 bits (3 bytes)
        trans.length = 0; // Don't send anything
        trans.__bindgen_anon_2.rx_buffer = rx_buf.as_mut_ptr() as *mut _;

        esp!(unsafe { spi_device_polling_transmit(self.spi_device, &mut trans) })?;

        // MT6701 SSI: first 14 bits are the angle value (MSB first)
        let raw = ((rx_buf[0] as u16) << 6) | ((rx_buf[1] as u16) >> 2);
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

impl Drop for Mt6701Encoder {
    fn drop(&mut self) {
        unsafe {
            spi_bus_remove_device(self.spi_device);
        }
    }
}
