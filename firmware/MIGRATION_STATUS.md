# NanoD Firmware Migration Status

C++ (Arduino/PlatformIO) → Rust (esp-idf-hal) for ESP32-S3

## What's Done — Phase 1: Motor Control

All motor + haptic core logic is ported and compiling for `xtensa-esp32s3-espidf`.
Pure math extracted into `nanod-math` crate with **46 host-side tests passing**.

| Module | Rust Location | Tests |
|--------|--------------|-------|
| FOC algorithm (SVPWM, inverse Park) | `nanod-math/src/motor/foc.rs` | 14 |
| PID controller | `nanod-math/src/haptic/pid.rs` | 10 |
| Haptic state machine (detents, bounds, hysteresis) | `nanod-math/src/haptic/state.rs` | 22 |
| Detent profiles, modes, events | `nanod-math/src/haptic/profile.rs` | — |
| MCPWM 3-phase driver (FFI) | `firmware/src/motor/driver.rs` | — |
| MT6701 SSI magnetic encoder (SPI) | `firmware/src/motor/encoder.rs` | — |
| NVS motor calibration persistence | `firmware/src/motor/calibration.rs` | — |
| GPIO pin map | `firmware/src/pins.rs` | — |
| FOC thread (Core 1, FreeRTOS) | `firmware/src/thread/foc_thread.rs` | — |
| Entry point | `firmware/src/main.rs` | — |

### C++ files fully replaced by Phase 1

| C++ File | Status |
|----------|--------|
| `src/haptic.h` / `src/haptic.cpp` | Replaced by `nanod-math` haptic modules |
| `src/haptic_api.h` | Replaced by `haptic/profile.rs` |
| `src/motor.h` | Constants in `motor/foc.rs`, pins in `pins.rs` |
| `include/nanofoc_d.h` | Replaced by `pins.rs` |
| `src/foc_thread.h` / `src/foc_thread.cpp` | Replaced by `thread/foc_thread.rs` |
| `src/thread_crtp.h` | Direct `xTaskCreatePinnedToCore` call |

---

## What's Left — By Phase

### Phase 2: COM Thread + Serial Protocol
Serial JSON command protocol, profile loading from SPIFFS.

| C++ File | What It Does |
|----------|-------------|
| `src/com_thread.h/cpp` | JSON serial protocol, dispatches config to other threads |
| `src/HapticCommander.h/cpp` | SimpleFOC register-based motor commands |
| `src/HapticProfileManager.h/cpp` | Load/save haptic profiles to SPIFFS as JSON |
| `src/HapticProfileUpdater.cpp` | Profile schema migration (v1 → v2) |
| `src/DeviceSettings.h/cpp` | Device settings persistence (Preferences + SPIFFS) |

**Deps needed**: `serde_json` or `embedded-json`, SPIFFS via esp-idf-svc

### Phase 3: HMI Thread (Buttons, LEDs, USB HID/MIDI)
Input handling, LED effects, USB device output.

| C++ File | What It Does |
|----------|-------------|
| `src/hmi_thread.h/cpp` | Buttons (AceButton×4), LEDs (FastLED 60+8), USB HID/MIDI/gamepad |
| `src/hmi_api.h` | Key mapping, knob mapping, HID/MIDI config structs |
| `src/led_api.h` | LED color/mode config |

**Deps needed**: GPIO input with debounce, WS2811 driver, TinyUSB FFI for HID/MIDI

### Phase 4: LCD Thread (LVGL Display)
Circular GC9A01 display with LVGL UI.

| C++ File | What It Does |
|----------|-------------|
| `src/lcd_thread.h/cpp` | LVGL display management, screen switching |
| `src/ui.h/c` | LVGL screen init |
| `src/ui_helpers.h/c` | UI utility functions |
| `src/screens/ui_*.c` (5 files) | Individual screen layouts |
| `src/fonts/ui_font_*.c` (7 files) | Embedded font data |
| `include/lv_conf.h` | LVGL config (16-bit color, 64KB buffer) |

**Deps needed**: LVGL Rust bindings or FFI, SPI display driver for GC9A01

### Phase 5: Audio (I2S)
Haptic feedback audio via I2S DAC.

| C++ File | What It Does |
|----------|-------------|
| `src/audio/audio.h/cpp` | I2S driver, WAV playback queue |
| `src/audio/audio_api.h` | Audio command/config types |
| `src/audio/WavData*.cpp` (3 files) | Embedded WAV sample data |
| `lib/XTI2S/` | Custom I2S audio library |

**Deps needed**: esp-idf-hal I2S driver, embedded WAV data

---

## External Library Dependencies

| C++ Library | Used For | Rust Strategy |
|-------------|----------|--------------|
| SimpleFOC v2.3.3 | Motor FOC control | **Replaced** — custom impl in `nanod-math` |
| SimpleFOCDrivers v1.0.7 | Encoder/driver abstractions | **Replaced** — `motor/encoder.rs`, `motor/driver.rs` |
| FastLED v3.6.0 | WS2811 RGB LEDs | Phase 3 — esp-idf RMT or smart-leds crate |
| LVGL v9.0.0 | Display GUI | Phase 4 — lvgl-rs bindings or C FFI |
| TFT_eSPI v2.5.43 | GC9A01 display driver | Phase 4 — mipidsi or custom SPI driver |
| ArduinoJson v7.0.2 | JSON serial protocol | Phase 2 — serde_json |
| AceButton v1.10.1 | Button debounce/events | Phase 3 — custom or esp-idf GPIO ISR |
| MIDI Library v5.0.2 | MIDI I/O | Phase 3 — midi-msg or custom |
| Adafruit TinyUSB v3.1.0 | USB HID/CDC | Phase 3 — TinyUSB FFI via esp-idf-sys |
| SparkFun STUSB4500 v1.1.5 | USB PD negotiation | Phase 3 — I2C register driver |

---

## Migration Score

| Category | Files | Ported | % |
|----------|-------|--------|---|
| Motor/Haptic core | 8 | 8 | **100%** |
| Threading | 8 | 2 | 25% |
| Settings/Profiles | 5 | 0 | 0% |
| HMI/Input | 4 | 0 | 0% |
| Display/UI | 15 | 0 | 0% |
| Audio | 8 | 0 | 0% |
| **Total** | **48** | **10** | **~21%** |

The hardest part (real-time motor control math) is done. The remaining work is mostly peripheral I/O and UI — higher-level, less timing-critical code.
