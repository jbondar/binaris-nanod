//! LVGL display driver for GC9A01.
//!
//! Initializes LVGL, creates a display with flush callback that sends
//! pixel data to the GC9A01 via SPI. Handles RGB565 byte swap and tick.

use core::ffi::c_void;
use esp_idf_sys::*;

use super::gc9a01::Gc9a01;

const DRAW_BUF_LINES: usize = 24;
const DRAW_BUF_SIZE: usize = 240 * DRAW_BUF_LINES * 2; // RGB565: 2 bytes/pixel

static mut DRAW_BUF: [u8; DRAW_BUF_SIZE] = [0; DRAW_BUF_SIZE];
static mut FLUSH_DISPLAY_PTR: *mut c_void = core::ptr::null_mut();

// Re-export LVGL constants with short names for use by display_thread
pub const PART_MAIN: lv_style_selector_t = _lv_part_t_LV_PART_MAIN;
pub const PART_INDICATOR: lv_style_selector_t = _lv_part_t_LV_PART_INDICATOR;
pub const PART_KNOB: lv_style_selector_t = _lv_part_t_LV_PART_KNOB;
pub const ALIGN_CENTER: lv_align_t = _lv_align_t_LV_ALIGN_CENTER as lv_align_t;
pub const ALIGN_TOP_MID: lv_align_t = _lv_align_t_LV_ALIGN_TOP_MID as lv_align_t;
pub const OPA_COVER: lv_opa_t = _lv_opa_t_LV_OPA_COVER as lv_opa_t;
pub const FLAG_CLICKABLE: lv_obj_flag_t = _lv_obj_flag_t_LV_OBJ_FLAG_CLICKABLE;

/// Initialize LVGL and create a display driver.
///
/// # Safety
/// `gc9a01_ptr` must point to a valid `Gc9a01` that outlives all LVGL operations.
/// All LVGL calls must happen from the same thread.
pub unsafe fn init_display(gc9a01_ptr: *mut c_void) -> *mut lv_display_t {
    FLUSH_DISPLAY_PTR = gc9a01_ptr;

    lv_init();
    lv_tick_set_cb(Some(tick_get_cb));

    let disp = lv_display_create(240, 240);
    lv_display_set_buffers(
        disp,
        core::ptr::addr_of_mut!(DRAW_BUF) as *mut c_void,
        core::ptr::null_mut(),
        DRAW_BUF_SIZE as u32,
        lv_display_render_mode_t_LV_DISPLAY_RENDER_MODE_PARTIAL,
    );
    lv_display_set_flush_cb(disp, Some(flush_cb));

    disp
}

/// LVGL tick callback — returns milliseconds since boot.
unsafe extern "C" fn tick_get_cb() -> u32 {
    (esp_timer_get_time() / 1000) as u32
}

/// LVGL flush callback — sends pixel data to GC9A01.
unsafe extern "C" fn flush_cb(
    disp: *mut lv_display_t,
    area: *const lv_area_t,
    px_map: *mut u8,
) {
    let gc9a01 = &mut *(FLUSH_DISPLAY_PTR as *mut Gc9a01<'static>);
    let area = &*area;

    let x1 = area.x1 as u16;
    let y1 = area.y1 as u16;
    let x2 = area.x2 as u16;
    let y2 = area.y2 as u16;

    let _ = gc9a01.set_window(x1, y1, x2, y2);

    let w = (x2 as usize + 1) - x1 as usize;
    let h = (y2 as usize + 1) - y1 as usize;
    let byte_count = w * h * 2;

    let data = core::slice::from_raw_parts(px_map, byte_count);
    let _ = gc9a01.write_pixels_raw(data);

    lv_display_flush_ready(disp);
}

/// Create lv_color_t from hex value (0xRRGGBB).
pub fn color_hex(hex: u32) -> lv_color_t {
    lv_color_t {
        red: ((hex >> 16) & 0xFF) as u8,
        green: ((hex >> 8) & 0xFF) as u8,
        blue: (hex & 0xFF) as u8,
    }
}

pub fn color_white() -> lv_color_t {
    lv_color_t {
        red: 255,
        green: 255,
        blue: 255,
    }
}

// Additional LVGL constants
pub const FLAG_HIDDEN: lv_obj_flag_t = _lv_obj_flag_t_LV_OBJ_FLAG_HIDDEN;
pub const COLOR_FORMAT_RGB565: lv_color_format_t =
    _lv_color_format_t_LV_COLOR_FORMAT_RGB565 as lv_color_format_t;
pub const ALIGN_BOTTOM_MID: lv_align_t = _lv_align_t_LV_ALIGN_BOTTOM_MID as lv_align_t;
pub const ALIGN_TOP_RIGHT: lv_align_t = _lv_align_t_LV_ALIGN_TOP_RIGHT as lv_align_t;
