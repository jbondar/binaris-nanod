//! Display thread: drives the GC9A01 circular LCD via LVGL.
//!
//! Two display modes:
//! - **Value**: arc indicator + position number + profile name (default)
//! - **Media**: album art + track info + volume/seek arc
//!
//! Runs on Core 0 alongside COM and HMI threads.

use esp_idf_hal::gpio::AnyInputPin;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::spi::{SpiDriver, SpiDriverConfig};
use esp_idf_sys::*;

use std::sync::mpsc::Receiver;

use super::gc9a01::{self, Gc9a01};
use super::lvgl_driver;
use crate::ipc::{AngleSnapshot, DisplayCommand, DisplayMode};

const DISPLAY_STACK_SIZE: usize = 32768;
const DISPLAY_PRIORITY: u32 = 1;
const DISPLAY_CORE: i32 = 0;

/// Album art: 240x240 RGB565 = 115,200 bytes (fills full circular display).
pub const ART_WIDTH: u32 = 240;
pub const ART_HEIGHT: u32 = 240;
pub const ART_BUF_SIZE: usize = (ART_WIDTH * ART_HEIGHT * 2) as usize;

/// Double-buffered album art. COM thread writes to LOAD buffer,
/// display thread swaps to DISPLAY buffer on MediaArtDone.
pub static mut ART_DISPLAY_PTR: *mut u8 = core::ptr::null_mut();
pub static mut ART_LOAD_PTR: *mut u8 = core::ptr::null_mut();

pub struct DisplayContext {
    pub angle_rx: Receiver<AngleSnapshot>,
    pub cmd_rx: Receiver<DisplayCommand>,
}

/// Value mode widgets.
struct ValueWidgets {
    container: *mut lv_obj_t,
    arc: *mut lv_obj_t,
    pos_label: *mut lv_obj_t,
    #[allow(dead_code)]
    name_label: *mut lv_obj_t,
}

/// Media mode widgets.
struct MediaWidgets {
    container: *mut lv_obj_t,
    arc: *mut lv_obj_t,
    canvas: *mut lv_obj_t,
    artist_label: *mut lv_obj_t,
    title_label: *mut lv_obj_t,
    mode_label: *mut lv_obj_t,
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
    log::info!("Display thread starting (LVGL)");

    let peripherals = unsafe { Peripherals::steal() };

    let spi_driver = SpiDriver::new(
        peripherals.spi3,
        peripherals.pins.gpio3,
        peripherals.pins.gpio4,
        None::<AnyInputPin>,
        &SpiDriverConfig::new(),
    )?;

    let mut gc9a01 = Gc9a01::new(
        &spi_driver,
        peripherals.pins.gpio6,
        peripherals.pins.gpio7,
        peripherals.pins.gpio2,
    )?;

    let mut backlight = esp_idf_hal::gpio::PinDriver::output(peripherals.pins.gpio5)?;
    backlight.set_high()?;
    log::info!("Display backlight ON");

    gc9a01.fill_screen(gc9a01::BLACK)?;

    let gc9a01_ptr = &mut gc9a01 as *mut Gc9a01<'_> as *mut core::ffi::c_void;
    let _disp = unsafe { lvgl_driver::init_display(gc9a01_ptr) };
    log::info!("LVGL initialized");

    let screen = unsafe { lv_display_get_screen_active(lv_display_get_default()) };

    // Only one UI exists at a time — delete and recreate on mode switch
    let mut value_ui: Option<ValueWidgets> = Some(unsafe { create_value_ui(screen) });
    let mut media_ui: Option<MediaWidgets> = None;
    log::info!("LVGL UI created — value mode");

    let mut current_mode = DisplayMode::Value;
    let mut last_pos: u16 = u16::MAX;

    loop {
        // Process display commands
        while let Ok(cmd) = ctx.cmd_rx.try_recv() {
            match cmd {
                DisplayCommand::SetMode(mode) => unsafe {
                    if mode == current_mode {
                        continue;
                    }
                    match mode {
                        DisplayMode::Value => {
                            if let Some(m) = media_ui.take() {
                                lv_obj_delete(m.container);
                            }
                            value_ui = Some(create_value_ui(screen));
                        }
                        DisplayMode::Media => {
                            if let Some(v) = value_ui.take() {
                                lv_obj_delete(v.container);
                            }
                            media_ui = Some(create_media_ui(screen));
                        }
                    }
                    current_mode = mode;
                    log::info!("Display mode: {:?}", current_mode);
                },

                DisplayCommand::MediaMeta {
                    title,
                    artist,
                    duration_s,
                    position_s,
                    playing,
                } => if let Some(ref m) = media_ui { unsafe {
                    let artist_c =
                        std::ffi::CString::new(artist).unwrap_or_default();
                    lv_label_set_text(m.artist_label, artist_c.as_ptr());

                    let title_c =
                        std::ffi::CString::new(title).unwrap_or_default();
                    lv_label_set_text(m.title_label, title_c.as_ptr());

                    if duration_s > 0 {
                        lv_arc_set_range(m.arc, 0, duration_s as i32);
                        lv_arc_set_value(m.arc, position_s as i32);
                    }

                    let _ = (playing, duration_s);
                }},

                DisplayCommand::MediaArtChunk { offset, data } => unsafe {
                    let buf_ptr = ART_LOAD_PTR;
                    if !buf_ptr.is_null() {
                        let buf_slice =
                            core::slice::from_raw_parts_mut(buf_ptr, ART_BUF_SIZE);
                        let start = offset as usize;
                        let end = (start + data.len()).min(ART_BUF_SIZE);
                        buf_slice[start..end].copy_from_slice(&data[..end - start]);
                    }
                },

                DisplayCommand::MediaArtStart => if let Some(ref m) = media_ui { unsafe {
                    lv_obj_add_flag(m.canvas, lvgl_driver::FLAG_HIDDEN);
                }},

                DisplayCommand::MediaArtDone => if let Some(ref m) = media_ui { unsafe {
                    let old_display = ART_DISPLAY_PTR;
                    ART_DISPLAY_PTR = ART_LOAD_PTR;
                    ART_LOAD_PTR = old_display;
                    lv_canvas_set_buffer(
                        m.canvas,
                        ART_DISPLAY_PTR as *mut core::ffi::c_void,
                        ART_WIDTH as i32,
                        ART_HEIGHT as i32,
                        lvgl_driver::COLOR_FORMAT_RGB565,
                    );
                    lv_obj_remove_flag(m.canvas, lvgl_driver::FLAG_HIDDEN);
                    lv_obj_invalidate(m.canvas);
                    log::info!("Album art swapped");
                }},
            }
        }

        // Update value mode from angle snapshots
        let mut latest: Option<AngleSnapshot> = None;
        while let Ok(snap) = ctx.angle_rx.try_recv() {
            latest = Some(snap);
        }

        if let Some(snap) = latest {
            if snap.current_pos != last_pos {
                last_pos = snap.current_pos;
                if let Some(ref v) = value_ui {
                    unsafe {
                        lv_arc_set_value(v.arc, last_pos as i32);
                        let text =
                            std::ffi::CString::new(format!("{}", last_pos)).unwrap();
                        lv_label_set_text(v.pos_label, text.as_ptr());
                    }
                }
            }
        }

        unsafe { lv_timer_handler() };
        unsafe { vTaskDelay(5) }; // ~50ms tick, must yield enough for IDLE0 watchdog
    }
}

/// Create value mode UI: arc + position number + profile name.
unsafe fn create_value_ui(screen: *mut lv_obj_t) -> ValueWidgets {
    let container = lv_obj_create(screen);
    lv_obj_set_size(container, 240, 240);
    lv_obj_align(container, lvgl_driver::ALIGN_CENTER, 0, 0);
    lv_obj_set_style_bg_color(container, lvgl_driver::color_hex(0x000000), 0);
    lv_obj_set_style_bg_opa(container, lvgl_driver::OPA_COVER, 0);
    lv_obj_set_style_border_width(container, 0, 0);
    lv_obj_set_style_pad_top(container, 0, 0);
    lv_obj_set_style_pad_bottom(container, 0, 0);
    lv_obj_set_style_pad_left(container, 0, 0);
    lv_obj_set_style_pad_right(container, 0, 0);
    lv_obj_remove_flag(container, lvgl_driver::FLAG_CLICKABLE);

    // Arc indicator
    let arc = lv_arc_create(container);
    lv_obj_set_size(arc, 220, 220);
    lv_obj_align(arc, lvgl_driver::ALIGN_CENTER, 0, 0);
    lv_arc_set_rotation(arc, 135);
    lv_arc_set_bg_angles(arc, 0, 270);
    lv_arc_set_range(arc, 0, 255);
    lv_arc_set_value(arc, 0);
    lv_obj_set_style_arc_color(arc, lvgl_driver::color_hex(0x333333), lvgl_driver::PART_MAIN);
    lv_obj_set_style_arc_width(arc, 12, lvgl_driver::PART_MAIN);
    lv_obj_set_style_arc_color(arc, lvgl_driver::color_hex(0xFF7D00), lvgl_driver::PART_INDICATOR);
    lv_obj_set_style_arc_width(arc, 12, lvgl_driver::PART_INDICATOR);
    lv_obj_set_style_arc_rounded(arc, true, lvgl_driver::PART_MAIN);
    lv_obj_set_style_arc_rounded(arc, true, lvgl_driver::PART_INDICATOR);
    lv_obj_remove_style(arc, core::ptr::null_mut(), lvgl_driver::PART_KNOB);
    lv_obj_remove_flag(arc, lvgl_driver::FLAG_CLICKABLE);

    // Position number
    let pos_label = lv_label_create(container);
    lv_obj_set_style_text_font(pos_label, &lv_font_montserrat_48 as *const _, 0);
    lv_obj_set_style_text_color(pos_label, lvgl_driver::color_white(), 0);
    lv_label_set_text(pos_label, b"0\0".as_ptr() as *const _);
    lv_obj_align(pos_label, lvgl_driver::ALIGN_CENTER, 0, 0);

    // Profile name
    let name_label = lv_label_create(container);
    lv_obj_set_style_text_font(name_label, &lv_font_montserrat_14 as *const _, 0);
    lv_obj_set_style_text_color(name_label, lvgl_driver::color_hex(0x888888), 0);
    lv_label_set_text(name_label, b"Default\0".as_ptr() as *const _);
    lv_obj_align(name_label, lvgl_driver::ALIGN_TOP_MID, 0, 30);

    ValueWidgets {
        container,
        arc,
        pos_label,
        name_label,
    }
}

/// Create media mode UI: album art + track info + arc.
unsafe fn create_media_ui(screen: *mut lv_obj_t) -> MediaWidgets {
    let container = lv_obj_create(screen);
    lv_obj_set_size(container, 240, 240);
    lv_obj_align(container, lvgl_driver::ALIGN_CENTER, 0, 0);
    lv_obj_set_style_bg_color(container, lvgl_driver::color_hex(0x000000), 0);
    lv_obj_set_style_bg_opa(container, lvgl_driver::OPA_COVER, 0);
    lv_obj_set_style_border_width(container, 0, 0);
    lv_obj_set_style_shadow_width(container, 0, 0);
    lv_obj_set_style_outline_width(container, 0, 0);
    lv_obj_set_style_pad_top(container, 0, 0);
    lv_obj_set_style_pad_bottom(container, 0, 0);
    lv_obj_set_style_pad_left(container, 0, 0);
    lv_obj_set_style_pad_right(container, 0, 0);
    // Clip entire container to circle
    lv_obj_set_style_radius(container, 120, 0);
    lv_obj_set_style_clip_corner(container, true, 0);
    lv_obj_set_scrollbar_mode(container, _lv_scrollbar_mode_t_LV_SCROLLBAR_MODE_OFF as u8);
    lv_obj_remove_flag(container, lvgl_driver::FLAG_CLICKABLE);

    // Album art canvas (240x240 RGB565, fills full circular display)
    // Placed first so it's behind labels and arc
    let canvas = lv_canvas_create(container);
    lv_canvas_set_buffer(
        canvas,
        ART_DISPLAY_PTR as *mut core::ffi::c_void,
        ART_WIDTH as i32,
        ART_HEIGHT as i32,
        lvgl_driver::COLOR_FORMAT_RGB565,
    );
    lv_obj_align(canvas, lvgl_driver::ALIGN_CENTER, 0, 0);

    // Arc (track progress, overlaid on art)
    let arc = lv_arc_create(container);
    lv_obj_set_size(arc, 236, 236);
    lv_obj_align(arc, lvgl_driver::ALIGN_CENTER, 0, 0);
    lv_arc_set_rotation(arc, 270);
    lv_arc_set_bg_angles(arc, 0, 360);
    lv_arc_set_range(arc, 0, 100);
    lv_arc_set_value(arc, 0);
    // Subtle track on top of art
    lv_obj_set_style_arc_color(arc, lvgl_driver::color_hex(0x000000), lvgl_driver::PART_MAIN);
    lv_obj_set_style_arc_opa(arc, 80, lvgl_driver::PART_MAIN);
    lv_obj_set_style_arc_width(arc, 4, lvgl_driver::PART_MAIN);
    lv_obj_set_style_arc_color(arc, lvgl_driver::color_white(), lvgl_driver::PART_INDICATOR);
    lv_obj_set_style_arc_opa(arc, 200, lvgl_driver::PART_INDICATOR);
    lv_obj_set_style_arc_width(arc, 4, lvgl_driver::PART_INDICATOR);
    lv_obj_set_style_arc_rounded(arc, true, lvgl_driver::PART_MAIN);
    lv_obj_set_style_arc_rounded(arc, true, lvgl_driver::PART_INDICATOR);
    lv_obj_remove_style(arc, core::ptr::null_mut(), lvgl_driver::PART_KNOB);
    lv_obj_remove_flag(arc, lvgl_driver::FLAG_CLICKABLE);

    // --- Pill-shaped label backgrounds for readability over album art ---

    // Artist name (top) — white text on dark semi-transparent pill
    let artist_label = lv_label_create(container);
    lv_obj_set_style_text_font(artist_label, &lv_font_montserrat_14 as *const _, 0);
    lv_obj_set_style_text_color(artist_label, lvgl_driver::color_white(), 0);
    lv_obj_set_style_bg_color(artist_label, lvgl_driver::color_hex(0x000000), 0);
    lv_obj_set_style_bg_opa(artist_label, 160, 0); // ~63% opaque
    lv_obj_set_style_radius(artist_label, 10, 0); // pill shape
    lv_obj_set_style_pad_left(artist_label, 10, 0);
    lv_obj_set_style_pad_right(artist_label, 10, 0);
    lv_obj_set_style_pad_top(artist_label, 4, 0);
    lv_obj_set_style_pad_bottom(artist_label, 4, 0);
    lv_label_set_text(artist_label, b"\0".as_ptr() as *const _);
    lv_obj_set_style_max_width(artist_label, 200, 0);
    lv_obj_set_style_text_align(artist_label, _lv_text_align_t_LV_TEXT_ALIGN_CENTER as u8, 0);
    lv_label_set_long_mode(artist_label, _lv_label_long_mode_t_LV_LABEL_LONG_SCROLL_CIRCULAR as u8);
    lv_obj_align(artist_label, lvgl_driver::ALIGN_TOP_MID, 0, 35);

    // Track title (bottom) — white text on dark semi-transparent pill
    let title_label = lv_label_create(container);
    lv_obj_set_style_text_font(title_label, &lv_font_montserrat_14 as *const _, 0);
    lv_obj_set_style_text_color(title_label, lvgl_driver::color_hex(0xEEEEEE), 0);
    lv_obj_set_style_bg_color(title_label, lvgl_driver::color_hex(0x000000), 0);
    lv_obj_set_style_bg_opa(title_label, 160, 0);
    lv_obj_set_style_radius(title_label, 10, 0);
    lv_obj_set_style_pad_left(title_label, 10, 0);
    lv_obj_set_style_pad_right(title_label, 10, 0);
    lv_obj_set_style_pad_top(title_label, 4, 0);
    lv_obj_set_style_pad_bottom(title_label, 4, 0);
    lv_label_set_text(title_label, b"\0".as_ptr() as *const _);
    lv_obj_set_style_max_width(title_label, 200, 0);
    lv_obj_set_style_text_align(title_label, _lv_text_align_t_LV_TEXT_ALIGN_CENTER as u8, 0);
    lv_label_set_long_mode(title_label, _lv_label_long_mode_t_LV_LABEL_LONG_SCROLL_CIRCULAR as u8);
    lv_obj_align(title_label, lvgl_driver::ALIGN_BOTTOM_MID, 0, -35);

    // Mode indicator (top-right) — small pill
    let mode_label = lv_label_create(container);
    lv_obj_set_style_text_font(mode_label, &lv_font_montserrat_14 as *const _, 0);
    lv_obj_set_style_text_color(mode_label, lvgl_driver::color_white(), 0);
    lv_obj_set_style_bg_color(mode_label, lvgl_driver::color_hex(0x4488FF), 0);
    lv_obj_set_style_bg_opa(mode_label, 200, 0);
    lv_obj_set_style_radius(mode_label, 8, 0);
    lv_obj_set_style_pad_left(mode_label, 6, 0);
    lv_obj_set_style_pad_right(mode_label, 6, 0);
    lv_obj_set_style_pad_top(mode_label, 2, 0);
    lv_obj_set_style_pad_bottom(mode_label, 2, 0);
    lv_label_set_text(mode_label, b"VOL\0".as_ptr() as *const _);
    lv_obj_align(mode_label, lvgl_driver::ALIGN_TOP_RIGHT, -25, 25);

    MediaWidgets {
        container,
        arc,
        canvas,
        artist_label,
        title_label,
        mode_label,
    }
}
