//! GPIO button polling for 4 hardware buttons with INPUT_PULLUP.

use esp_idf_hal::gpio::{Input, PinDriver, Pull};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_sys::EspError;

/// Reads 4 button GPIO pins configured as INPUT_PULLUP.
pub struct ButtonGpio<'a> {
    pins: [PinDriver<'a, Input>; 4],
}

impl<'a> ButtonGpio<'a> {
    /// Initialize button GPIOs. Uses `Peripherals::steal()` (same as other threads).
    pub fn new() -> Result<Self, EspError> {
        let peripherals = unsafe { Peripherals::steal() };

        let btn_a = PinDriver::input(peripherals.pins.gpio41, Pull::Up)?;
        let btn_b = PinDriver::input(peripherals.pins.gpio40, Pull::Up)?;
        let btn_c = PinDriver::input(peripherals.pins.gpio45, Pull::Up)?;
        let btn_d = PinDriver::input(peripherals.pins.gpio46, Pull::Up)?;

        Ok(Self {
            pins: [btn_a, btn_b, btn_c, btn_d],
        })
    }

    /// Read raw GPIO levels. `true` = high (released for INPUT_PULLUP), `false` = low (pressed).
    pub fn read_levels(&self) -> [bool; 4] {
        [
            self.pins[0].is_high(),
            self.pins[1].is_high(),
            self.pins[2].is_high(),
            self.pins[3].is_high(),
        ]
    }
}
