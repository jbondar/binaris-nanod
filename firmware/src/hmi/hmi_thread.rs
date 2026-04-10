//! HMI thread: buttons, LEDs, and (future) USB HID/MIDI.
//!
//! Runs on Core 0 at 10ms loop (100Hz), matching the C++ hmi_thread.
//! - Polls 4 GPIO buttons, debounces, emits key events to COM thread
//! - Renders ring + button LEDs via RMT WS2811 driver
//! - Receives config updates from COM thread and angle data from FOC thread

use esp_idf_sys::*;

use nanod_math::hmi::button::{ButtonDebouncer, ButtonEventType};
use nanod_math::led::button_leds::{self, BUTTON_LED_COUNT};
use nanod_math::led::ring::{self, RING_LED_COUNT};
use nanod_math::led::types::{LedConfig, Rgb};
use nanod_math::protocol::command::{KeyData, KeyEvent};

use crate::ipc::{AngleSnapshot, HmiCommand, HmiContext};
use crate::pins;

use super::buttons::ButtonGpio;
use super::leds::Ws2811Driver;

const HMI_STACK_SIZE: usize = 8192;
const HMI_PRIORITY: u32 = 1;
const HMI_CORE: i32 = 0;
const LOOP_DELAY_MS: u32 = 10; // 100Hz
const LED_SHOW_INTERVAL_MS: u32 = 16; // ~60fps

/// Spawn the HMI thread on Core 0.
pub fn spawn_hmi_thread(ctx: HmiContext) {
    let ctx_ptr = Box::into_raw(Box::new(ctx)) as *mut core::ffi::c_void;
    unsafe {
        let mut handle: TaskHandle_t = core::ptr::null_mut();
        xTaskCreatePinnedToCore(
            Some(hmi_task),
            b"hmi\0".as_ptr() as *const _,
            HMI_STACK_SIZE as u32,
            ctx_ptr,
            HMI_PRIORITY,
            &mut handle,
            HMI_CORE,
        );
    }
}

unsafe extern "C" fn hmi_task(arg: *mut core::ffi::c_void) {
    let ctx = unsafe { *Box::from_raw(arg as *mut HmiContext) };
    if let Err(e) = hmi_task_inner(ctx) {
        log::error!("HMI task failed: {:?}", e);
    }
    loop {
        vTaskDelay(1000);
    }
}

fn hmi_task_inner(ctx: HmiContext) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("HMI thread starting");

    // --- Initialize hardware ---
    let button_gpio = ButtonGpio::new()?;
    let mut led_driver = Ws2811Driver::new(pins::LED_A, pins::LED_B)?;

    // --- State ---
    let mut debouncer = ButtonDebouncer::new();
    let mut led_config = LedConfig::default();
    let mut brightness: u8 = 200;
    let mut orientation: u8 = 0;

    let mut ring_buf = [Rgb::default(); RING_LED_COUNT];
    let mut button_buf = [Rgb::default(); BUTTON_LED_COUNT];

    let mut latest_angle = AngleSnapshot {
        shaft_angle: 0.0,
        current_pos: 0,
    };

    let mut last_led_show_ms: u32 = 0;

    log::info!("HMI thread initialized, entering main loop");

    loop {
        let now_ms = (unsafe { esp_timer_get_time() } / 1000) as u32;

        // --- 1. Process config updates from COM ---
        while let Ok(cmd) = ctx.cmd_rx.try_recv() {
            match cmd {
                HmiCommand::UpdateLedConfig(cfg) => {
                    log::info!("HMI: LED config updated");
                    led_config = cfg;
                }
                HmiCommand::UpdateSettings {
                    brightness: b,
                    orientation: o,
                } => {
                    brightness = b;
                    orientation = o;
                    log::info!("HMI: settings updated — brightness={b}, orientation={o}");
                }
            }
        }

        // --- 2. Get latest angle from FOC ---
        while let Ok(snap) = ctx.angle_rx.try_recv() {
            latest_angle = snap;
        }

        // --- 3. Poll buttons and debounce ---
        let levels = button_gpio.read_levels();
        let events = debouncer.update(levels, now_ms);

        for evt in &events {
            let state = match evt.event_type {
                ButtonEventType::Pressed => "pressed",
                ButtonEventType::Released => "released",
            };
            let key_event = KeyEvent {
                key: KeyData {
                    num: evt.index,
                    state: state.to_string(),
                },
            };
            let _ = ctx.key_tx.try_send(key_event);
        }

        // --- 4. Render LEDs (throttled to ~60fps) ---
        if now_ms.wrapping_sub(last_led_show_ms) >= LED_SHOW_INTERVAL_MS {
            last_led_show_ms = now_ms;

            if led_config.enabled {
                // Ring: map knob position to LED index, apply halves pointer
                let led_index =
                    ring::position_to_led_index(latest_angle.current_pos, 0, 255);
                let offset = ring::orientation_to_offset(orientation);

                // Apply brightness to colors
                let effective_brightness = brightness.min(led_config.brightness);
                let pointer = led_config.pointer_col.scaled(effective_brightness);
                let primary = led_config.primary_col.scaled(effective_brightness);
                let secondary = led_config.secondary_col.scaled(effective_brightness);

                ring::halves_pointer(
                    &mut ring_buf,
                    led_index,
                    offset,
                    pointer,
                    primary,
                    secondary,
                );

                // Button LEDs
                button_leds::update_button_leds(
                    &mut button_buf,
                    debouncer.key_state(),
                    &led_config.button_colors,
                );
                // Scale button LEDs by brightness
                for px in button_buf.iter_mut() {
                    *px = px.scaled(effective_brightness);
                }

                let _ = led_driver.show_ring(&ring_buf);
                let _ = led_driver.show_buttons(&button_buf);
            }
        }

        // --- 5. Sleep ---
        unsafe { vTaskDelay(LOOP_DELAY_MS) };
    }
}
