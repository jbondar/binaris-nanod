//! Display thread: drives the GC9A01 circular LCD.
//!
//! Shows the current detent position as a large number.
//! Runs on Core 0 alongside COM and HMI threads.

use esp_idf_hal::gpio::AnyInputPin;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::spi::{SpiDriver, SpiDriverConfig};
use esp_idf_sys::*;

use std::sync::mpsc::Receiver;

use super::gc9a01::{self, Gc9a01, BLACK, ORANGE, WHITE};
use crate::ipc::AngleSnapshot;
use crate::pins;

const DISPLAY_STACK_SIZE: usize = 16384;
const DISPLAY_PRIORITY: u32 = 1;
const DISPLAY_CORE: i32 = 0;

pub struct DisplayContext {
    pub angle_rx: Receiver<AngleSnapshot>,
}

pub fn spawn_display_thread(ctx: DisplayContext) {
    let ctx_ptr = Box::into_raw(Box::new(ctx)) as *mut core::ffi::c_void;
    unsafe {
        let mut handle: TaskHandle_t = core::ptr::null_mut();
        xTaskCreatePinnedToCore(
            Some(display_task),
            b"lcd\0".as_ptr() as *const _,
            DISPLAY_STACK_SIZE as u32,
            ctx_ptr,
            DISPLAY_PRIORITY,
            &mut handle,
            DISPLAY_CORE,
        );
    }
}

unsafe extern "C" fn display_task(arg: *mut core::ffi::c_void) {
    let ctx = unsafe { *Box::from_raw(arg as *mut DisplayContext) };
    if let Err(e) = display_task_inner(ctx) {
        log::error!("Display task failed: {:?}", e);
    }
    loop {
        vTaskDelay(1000);
    }
}

fn display_task_inner(ctx: DisplayContext) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Display thread starting");

    let peripherals = unsafe { Peripherals::steal() };

    // SPI bus for display (separate from encoder SPI)
    let spi_driver = SpiDriver::new(
        peripherals.spi3,        // Use SPI3 (encoder is on SPI2)
        peripherals.pins.gpio3,  // SCLK
        peripherals.pins.gpio4,  // MOSI
        None::<AnyInputPin>,     // No MISO
        &SpiDriverConfig::new(),
    )?;

    let mut display = Gc9a01::new(
        &spi_driver,
        peripherals.pins.gpio6, // CS
        peripherals.pins.gpio7, // DC
        peripherals.pins.gpio2, // RST
    )?;

    // Turn on backlight (GPIO5, active high)
    let mut backlight = esp_idf_hal::gpio::PinDriver::output(peripherals.pins.gpio5)?;
    backlight.set_high()?;
    log::info!("Display backlight ON");

    // Clear screen
    display.fill_screen(BLACK)?;
    log::info!("Display initialized — showing position");

    let mut last_pos: u16 = u16::MAX; // force initial draw

    loop {
        // Drain channel, keep latest
        let mut latest: Option<AngleSnapshot> = None;
        while let Ok(snap) = ctx.angle_rx.try_recv() {
            latest = Some(snap);
        }

        if let Some(snap) = latest {
            if snap.current_pos != last_pos {
                last_pos = snap.current_pos;
                display.draw_number(last_pos, ORANGE, BLACK)?;
            }
        }

        // ~30fps update rate
        unsafe { vTaskDelay(3) }; // 3 ticks = 30ms at 100Hz
    }
}
