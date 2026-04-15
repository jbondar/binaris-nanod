//! Minimal GC9A01 240x240 circular display driver over SPI.
//!
//! RGB565 color format, 80MHz SPI. No LVGL — direct framebuffer writes.
//! Enough for debug display (large numbers, solid colors).

use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::{Output, PinDriver};
use esp_idf_hal::spi::{self, SpiConfig, SpiDeviceDriver};
use esp_idf_hal::units::Hertz;
use esp_idf_sys::EspError;

use embedded_hal::spi::SpiDevice;

pub const WIDTH: u16 = 240;
pub const HEIGHT: u16 = 240;

/// RGB565 color helpers.
pub fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | ((b as u16) >> 3)
}

pub const BLACK: u16 = 0x0000;
pub const WHITE: u16 = 0xFFFF;
pub const ORANGE: u16 = 0xFBE0; // ~0xFF7D00

pub struct Gc9a01<'a> {
    spi: SpiDeviceDriver<'a, &'a spi::SpiDriver<'a>>,
    dc: PinDriver<'a, Output>,
    rst: PinDriver<'a, Output>,
}

impl<'a> Gc9a01<'a> {
    pub fn new(
        spi_driver: &'a spi::SpiDriver<'a>,
        cs_pin: impl esp_idf_hal::gpio::OutputPin + 'a,
        dc_pin: impl esp_idf_hal::gpio::OutputPin + 'a,
        rst_pin: impl esp_idf_hal::gpio::OutputPin + 'a,
    ) -> Result<Self, EspError> {
        let config = SpiConfig::new()
            .baudrate(Hertz(40_000_000)) // 40MHz (safe starting point)
            .data_mode(embedded_hal::spi::MODE_0);

        let spi = SpiDeviceDriver::new(spi_driver, Some(cs_pin), &config)?;
        let dc = PinDriver::output(dc_pin)?;
        let rst = PinDriver::output(rst_pin)?;

        let mut display = Self { spi, dc, rst };
        display.init()?;
        Ok(display)
    }

    fn init(&mut self) -> Result<(), EspError> {
        // Hardware reset
        self.rst.set_high()?;
        FreeRtos::delay_ms(10);
        self.rst.set_low()?;
        FreeRtos::delay_ms(10);
        self.rst.set_high()?;
        FreeRtos::delay_ms(120);

        // GC9A01 init sequence (from datasheet + common init code)
        self.cmd(0xEF)?; // Inter register enable 2
        self.cmd(0xEB)?;
        self.data(&[0x14])?;

        self.cmd(0xFE)?; // Inter register enable 1
        self.cmd(0xEF)?; // Inter register enable 2

        self.cmd(0xEB)?;
        self.data(&[0x14])?;

        self.cmd(0x84)?;
        self.data(&[0x40])?;

        self.cmd(0x85)?;
        self.data(&[0xFF])?;

        self.cmd(0x86)?;
        self.data(&[0xFF])?;

        self.cmd(0x87)?;
        self.data(&[0xFF])?;

        self.cmd(0x88)?;
        self.data(&[0x0A])?;

        self.cmd(0x89)?;
        self.data(&[0x21])?;

        self.cmd(0x8A)?;
        self.data(&[0x00])?;

        self.cmd(0x8B)?;
        self.data(&[0x80])?;

        self.cmd(0x8C)?;
        self.data(&[0x01])?;

        self.cmd(0x8D)?;
        self.data(&[0x01])?;

        self.cmd(0x8E)?;
        self.data(&[0xFF])?;

        self.cmd(0x8F)?;
        self.data(&[0xFF])?;

        self.cmd(0xB6)?; // Display function control
        self.data(&[0x00, 0x00])?;

        self.cmd(0x36)?; // Memory access control
        // Bit 6 = Column order (mirror X), Bit 3 = RGB order
        self.data(&[0x48])?; // Mirror X + RGB

        self.cmd(0x3A)?; // Pixel format
        self.data(&[0x05])?; // RGB565

        self.cmd(0x90)?;
        self.data(&[0x08, 0x08, 0x08, 0x08])?;

        self.cmd(0xBD)?;
        self.data(&[0x06])?;

        self.cmd(0xBC)?;
        self.data(&[0x00])?;

        self.cmd(0xFF)?;
        self.data(&[0x60, 0x01, 0x04])?;

        self.cmd(0xC3)?; // Voltage regulator 1a
        self.data(&[0x13])?;
        self.cmd(0xC4)?; // Voltage regulator 1b
        self.data(&[0x13])?;

        self.cmd(0xC9)?; // Voltage regulator 2a
        self.data(&[0x22])?;

        self.cmd(0xBE)?;
        self.data(&[0x11])?;

        self.cmd(0xE1)?;
        self.data(&[0x10, 0x0E])?;

        self.cmd(0xDF)?;
        self.data(&[0x21, 0x0C, 0x02])?;

        // Gamma
        self.cmd(0xF0)?;
        self.data(&[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A])?;
        self.cmd(0xF1)?;
        self.data(&[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F])?;
        self.cmd(0xF2)?;
        self.data(&[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A])?;
        self.cmd(0xF3)?;
        self.data(&[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F])?;

        self.cmd(0xED)?;
        self.data(&[0x1B, 0x0B])?;

        self.cmd(0xAE)?;
        self.data(&[0x77])?;

        self.cmd(0xCD)?;
        self.data(&[0x63])?;

        self.cmd(0x70)?;
        self.data(&[0x07, 0x07, 0x04, 0x0E, 0x0F, 0x09, 0x07, 0x08, 0x03])?;

        self.cmd(0xE8)?;
        self.data(&[0x34])?;

        self.cmd(0x62)?;
        self.data(&[
            0x18, 0x0D, 0x71, 0xED, 0x70, 0x70, 0x18, 0x0F, 0x71, 0xEF, 0x70, 0x70,
        ])?;

        self.cmd(0x63)?;
        self.data(&[
            0x18, 0x11, 0x71, 0xF1, 0x70, 0x70, 0x18, 0x13, 0x71, 0xF3, 0x70, 0x70,
        ])?;

        self.cmd(0x64)?;
        self.data(&[0x28, 0x29, 0xF1, 0x01, 0xF1, 0x00, 0x07])?;

        self.cmd(0x66)?;
        self.data(&[0x3C, 0x00, 0xCD, 0x67, 0x45, 0x45, 0x10, 0x00, 0x00, 0x00])?;

        self.cmd(0x67)?;
        self.data(&[0x00, 0x3C, 0x00, 0x00, 0x00, 0x01, 0x54, 0x10, 0x32, 0x98])?;

        self.cmd(0x74)?;
        self.data(&[0x10, 0x85, 0x80, 0x00, 0x00, 0x4E, 0x00])?;

        self.cmd(0x98)?;
        self.data(&[0x3E, 0x07])?;

        self.cmd(0x35)?; // Tearing effect on
        self.cmd(0x21)?; // Display inversion on

        self.cmd(0x11)?; // Sleep out
        FreeRtos::delay_ms(120);

        self.cmd(0x29)?; // Display on
        FreeRtos::delay_ms(20);

        log::info!("GC9A01 display initialized");
        Ok(())
    }

    fn cmd(&mut self, cmd: u8) -> Result<(), EspError> {
        self.dc.set_low()?;
        self.spi
            .write(&[cmd])
            .map_err(|_| EspError::from_infallible::<{ esp_idf_sys::ESP_FAIL }>())?;
        Ok(())
    }

    fn data(&mut self, data: &[u8]) -> Result<(), EspError> {
        self.dc.set_high()?;
        self.spi
            .write(data)
            .map_err(|_| EspError::from_infallible::<{ esp_idf_sys::ESP_FAIL }>())?;
        Ok(())
    }

    /// Set the drawing window.
    pub fn set_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) -> Result<(), EspError> {
        self.cmd(0x2A)?; // Column address set
        self.data(&[
            (x0 >> 8) as u8,
            (x0 & 0xFF) as u8,
            (x1 >> 8) as u8,
            (x1 & 0xFF) as u8,
        ])?;
        self.cmd(0x2B)?; // Row address set
        self.data(&[
            (y0 >> 8) as u8,
            (y0 & 0xFF) as u8,
            (y1 >> 8) as u8,
            (y1 & 0xFF) as u8,
        ])?;
        self.cmd(0x2C)?; // Memory write
        Ok(())
    }

    /// Write pixel data (RGB565, big-endian).
    pub fn write_pixels(&mut self, pixels: &[u16]) -> Result<(), EspError> {
        // Convert to bytes (big-endian for GC9A01)
        // Write in chunks to avoid huge stack allocation
        let mut buf = [0u8; 512];
        let mut offset = 0;
        while offset < pixels.len() {
            let chunk = (pixels.len() - offset).min(buf.len() / 2);
            for i in 0..chunk {
                buf[i * 2] = (pixels[offset + i] >> 8) as u8;
                buf[i * 2 + 1] = (pixels[offset + i] & 0xFF) as u8;
            }
            self.dc.set_high()?;
            self.spi
                .write(&buf[..chunk * 2])
                .map_err(|_| EspError::from_infallible::<{ esp_idf_sys::ESP_FAIL }>())?;
            offset += chunk;
        }
        Ok(())
    }

    /// Fill entire screen with a single color.
    pub fn fill_screen(&mut self, color: u16) -> Result<(), EspError> {
        self.set_window(0, 0, WIDTH - 1, HEIGHT - 1)?;
        let row = [color; 240];
        for _ in 0..HEIGHT {
            self.write_pixels(&row)?;
        }
        Ok(())
    }

    /// Fill a rectangle with a single color.
    pub fn fill_rect(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        color: u16,
    ) -> Result<(), EspError> {
        self.set_window(x, y, x + w - 1, y + h - 1)?;
        let row = vec![color; w as usize];
        for _ in 0..h {
            self.write_pixels(&row)?;
        }
        Ok(())
    }

    /// Draw a large digit (0-9) at position. Simple 5x7 bitmap scaled up.
    /// `scale` multiplies the base 5x7 grid size.
    pub fn draw_digit(
        &mut self,
        x: u16,
        y: u16,
        digit: u8,
        scale: u16,
        fg: u16,
        bg: u16,
    ) -> Result<(), EspError> {
        let bitmap = DIGIT_BITMAPS[digit.min(9) as usize];
        let w = 5 * scale;
        let h = 7 * scale;
        self.set_window(x, y, x + w - 1, y + h - 1)?;

        let mut row_buf = vec![bg; w as usize];
        for row in 0..7u16 {
            // Build one row of the digit
            for col in 0..5u16 {
                let on = (bitmap[row as usize] >> (4 - col)) & 1 == 1;
                let color = if on { fg } else { bg };
                for sx in 0..scale {
                    row_buf[(col * scale + sx) as usize] = color;
                }
            }
            // Write `scale` copies of this row
            for _ in 0..scale {
                self.write_pixels(&row_buf)?;
            }
        }
        Ok(())
    }

    /// Draw a number (up to 3 digits) centered on screen.
    /// Renders the entire region in one pass to avoid visible scanline flicker.
    pub fn draw_number(&mut self, value: u16, fg: u16, bg: u16) -> Result<(), EspError> {
        let scale: u16 = 8;
        let digit_w = 5 * scale;  // 40px per digit
        let digit_h = 7 * scale;  // 56px height
        let gap = scale * 2;      // 16px gap

        let digits: Vec<u8> = if value == 0 {
            vec![0]
        } else {
            let mut v = value;
            let mut d = Vec::new();
            while v > 0 {
                d.push((v % 10) as u8);
                v /= 10;
            }
            d.reverse();
            d
        };

        // Render into full-width region so old digits are cleared
        let region_w = WIDTH;
        let region_h = digit_h;
        let start_y = (HEIGHT - region_h) / 2;

        let total_w = digits.len() as u16 * digit_w + (digits.len() as u16).saturating_sub(1) * gap;
        let start_x = (WIDTH - total_w) / 2;

        self.set_window(0, start_y, region_w - 1, start_y + region_h - 1)?;

        // Render one row at a time across the full width
        let mut row_buf = vec![bg; region_w as usize];

        for bitmap_row in 0..7u16 {
            // Build this row: background + digit pixels
            for px in row_buf.iter_mut() {
                *px = bg;
            }

            for (di, &d) in digits.iter().enumerate() {
                let bitmap = DIGIT_BITMAPS[d.min(9) as usize];
                let dx = start_x + di as u16 * (digit_w + gap);

                for col in 0..5u16 {
                    let on = (bitmap[bitmap_row as usize] >> (4 - col)) & 1 == 1;
                    if on {
                        for sx in 0..scale {
                            let x = (dx + col * scale + sx) as usize;
                            if x < region_w as usize {
                                row_buf[x] = fg;
                            }
                        }
                    }
                }
            }

            // Write this row `scale` times (vertical scaling)
            for _ in 0..scale {
                self.write_pixels(&row_buf)?;
            }
        }

        Ok(())
    }
}

/// 5x7 bitmap font for digits 0-9. Each byte is one row, MSB=left.
const DIGIT_BITMAPS: [[u8; 7]; 10] = [
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111], // 2
    [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110], // 3
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // 5
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // 7
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // 9
];
