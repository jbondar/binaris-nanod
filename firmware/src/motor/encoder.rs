use core::f32::consts::PI;

use esp_idf_sys::*;

const TWO_PI: f32 = 2.0 * PI;
const MT6701_RESOLUTION: f32 = 16384.0; // 14-bit

// ESP32-S3 SPI2 register offsets (from TRM Chapter 26)
const SPI2_BASE: u32 = 0x6002_4000; // DR_REG_SPI2_BASE
const SPI_CMD_REG: u32 = SPI2_BASE + 0x00;
const SPI_USER_REG: u32 = SPI2_BASE + 0x10;
const SPI_USER1_REG: u32 = SPI2_BASE + 0x14;
const SPI_MS_DLEN_REG: u32 = SPI2_BASE + 0x1C;
const SPI_MISC_REG: u32 = SPI2_BASE + 0x20;
const SPI_CLK_REG: u32 = SPI2_BASE + 0x0C;
const SPI_W0_REG: u32 = SPI2_BASE + 0x58;
const SPI_PIN_REG: u32 = SPI2_BASE + 0x24;

/// MT6701 magnetic angle sensor via SSI (SPI mode 3).
///
/// Uses direct SPI register access for maximum speed (~5µs per read).
/// After initial setup via esp-idf SPI driver, reads bypass the driver
/// entirely for the hot loop.
pub struct Mt6701Encoder {
    spi_device: spi_device_handle_t,
    prev_raw: u16,
    pub full_rotations: i32,
    angle_offset: f32,
    use_direct: bool, // true after setup is complete
}

#[inline(always)]
fn reg_write(addr: u32, val: u32) {
    unsafe { core::ptr::write_volatile(addr as *mut u32, val) };
}

#[inline(always)]
fn reg_read(addr: u32) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

impl Mt6701Encoder {
    /// Initialize SPI device for encoder.
    pub fn new(
        host: spi_host_device_t,
        cs_pin: i32,
        sclk_pin: i32,
        miso_pin: i32,
        mosi_pin: i32,
    ) -> Result<Self, EspError> {
        // Configure SPI bus via ESP-IDF driver (for proper pin muxing)
        let mut bus_config: spi_bus_config_t = unsafe { core::mem::zeroed() };
        bus_config.__bindgen_anon_1.mosi_io_num = mosi_pin;
        bus_config.__bindgen_anon_2.miso_io_num = miso_pin;
        bus_config.sclk_io_num = sclk_pin;
        bus_config.__bindgen_anon_3.quadwp_io_num = -1;
        bus_config.__bindgen_anon_4.quadhd_io_num = -1;
        bus_config.max_transfer_sz = 32;
        bus_config.flags = SPICOMMON_BUSFLAG_MASTER;

        esp!(unsafe { spi_bus_initialize(host, &bus_config, 0) })?;

        // Add device to configure CS pin and clock
        let mut dev_config: spi_device_interface_config_t = unsafe { core::mem::zeroed() };
        dev_config.clock_speed_hz = 10_000_000;
        dev_config.mode = 3;
        dev_config.spics_io_num = cs_pin;
        dev_config.queue_size = 1;
        dev_config.flags = SPI_DEVICE_HALFDUPLEX;

        let mut spi_device: spi_device_handle_t = core::ptr::null_mut();
        esp!(unsafe { spi_bus_add_device(host, &dev_config, &mut spi_device) })?;

        // Do one transaction through the driver to fully configure the hardware
        let mut rx_buf = [0u8; 4];
        let mut trans: spi_transaction_t = unsafe { core::mem::zeroed() };
        trans.rxlength = 24;
        trans.length = 0;
        trans.__bindgen_anon_2.rx_buffer = rx_buf.as_mut_ptr() as *mut _;
        esp!(unsafe { spi_device_polling_transmit(spi_device, &mut trans) })?;

        // Now compare driver read vs direct register read to verify byte order
        let driver_raw = ((rx_buf[0] as u16) << 6) | ((rx_buf[1] as u16) >> 2);
        let driver_raw = driver_raw & 0x3FFF;

        // Do a direct register read for comparison
        reg_write(SPI_W0_REG, 0);
        reg_write(SPI_CMD_REG, 1 << 18);
        while reg_read(SPI_CMD_REG) & (1 << 18) != 0 {}
        let w0 = reg_read(SPI_W0_REG);

        log::info!(
            "MT6701 init: driver=[{:#04X},{:#04X},{:#04X}] raw14={}, W0={:#010X}",
            rx_buf[0], rx_buf[1], rx_buf[2], driver_raw, w0
        );

        // Acquire the SPI bus permanently for this device — eliminates per-transaction
        // locking overhead. This is safe because only the FOC thread uses SPI2.
        esp!(unsafe { spi_device_acquire_bus(spi_device, u32::MAX) })?;
        log::info!("MT6701: SPI bus acquired for permanent use");

        Ok(Self {
            spi_device,
            prev_raw: 0,
            full_rotations: 0,
            angle_offset: 0.0,
            use_direct: false, // use driver with pre-acquired bus
        })
    }

    /// Read the current angle in radians (continuous, multi-turn).
    #[inline(always)]
    pub fn read_angle(&mut self) -> Result<f32, EspError> {
        // Use driver-based polling read — direct register read doesn't work yet
        let raw = self.read_raw_driver()?;

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

    /// Direct register SPI read — bypasses ESP-IDF driver for speed.
    /// Falls back to driver-based read if direct read returns invalid data.
    #[inline(always)]
    fn read_raw_direct(&self) -> u16 {
        // Clear W0 receive buffer
        reg_write(SPI_W0_REG, 0);

        // Start transfer (USR command)
        reg_write(SPI_CMD_REG, 1 << 18);

        // Busy-wait for completion
        while reg_read(SPI_CMD_REG) & (1 << 18) != 0 {}

        // Read result — ESP32-S3 stores received bits in W0
        // For 24 bits received in half-duplex MISO mode:
        // Data is stored starting from bit 0 of W0, MSB first in byte order
        let w0 = reg_read(SPI_W0_REG);

        // The byte order depends on the SPI_RD_BYTE_ORDER bit.
        // Default (0) = little-endian: first received byte in lowest byte of W0
        let byte0 = (w0 & 0xFF) as u8;
        let byte1 = ((w0 >> 8) & 0xFF) as u8;

        let raw = ((byte0 as u16) << 6) | ((byte1 as u16) >> 2);
        raw & 0x3FFF
    }

    /// Driver-based SPI read (for calibration, non-hot-path use).
    fn read_raw_driver(&mut self) -> Result<u16, EspError> {
        let mut rx_buf = [0u8; 4];
        let mut trans: spi_transaction_t = unsafe { core::mem::zeroed() };
        trans.rxlength = 24;
        trans.length = 0;
        trans.__bindgen_anon_2.rx_buffer = rx_buf.as_mut_ptr() as *mut _;

        esp!(unsafe { spi_device_polling_transmit(self.spi_device, &mut trans) })?;

        let raw = ((rx_buf[0] as u16) << 6) | ((rx_buf[1] as u16) >> 2);
        Ok(raw & 0x3FFF)
    }

    /// Get the mechanical angle in 0-2PI range (single rotation, no multi-turn).
    #[inline(always)]
    pub fn get_mechanical_angle(&self) -> f32 {
        self.prev_raw as f32 / MT6701_RESOLUTION * TWO_PI
    }

    /// Set the current position as the zero reference.
    pub fn set_zero(&mut self) -> Result<(), EspError> {
        let raw = self.read_raw_driver()?;
        self.angle_offset = raw as f32 / MT6701_RESOLUTION * TWO_PI;
        self.full_rotations = 0;
        self.prev_raw = raw;
        Ok(())
    }

    /// Get the raw angle for calibration purposes.
    pub fn get_raw_angle(&mut self) -> Result<f32, EspError> {
        let raw = self.read_raw_driver()?;
        Ok(raw as f32 / MT6701_RESOLUTION * TWO_PI)
    }
}

impl Drop for Mt6701Encoder {
    fn drop(&mut self) {
        unsafe {
            spi_device_release_bus(self.spi_device);
            spi_bus_remove_device(self.spi_device);
        }
    }
}
