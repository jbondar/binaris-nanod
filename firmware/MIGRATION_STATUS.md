# NanoD Firmware Migration Status

C++ (Arduino/PlatformIO) → Rust (esp-idf-hal) for ESP32-S3

## What's Done — Phases 1-3A

All motor/haptic core, COM serial protocol, profile management, and HMI (buttons + LEDs) are ported and compiling for `xtensa-esp32s3-espidf`.
Pure math extracted into `nanod-math` crate with **115 host-side tests passing**.

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
| JSON protocol (command/event types) | `nanod-math/src/protocol/command.rs` | 12 |
| Protocol parsing | `nanod-math/src/protocol/parse.rs` | 12 |
| Protocol serialization | `nanod-math/src/protocol/serialize.rs` | 7 |
| Profile manager (dirty tracking) | `nanod-math/src/profile/manager.rs` | 12 |
| COM thread (Core 0, JSON serial) | `firmware/src/com/com_thread.rs` | — |
| Command dispatcher | `firmware/src/com/dispatch.rs` | — |
| LED rendering (ring halves pointer) | `nanod-math/src/led/ring.rs` | 13 |
| LED rendering (button LEDs) | `nanod-math/src/led/button_leds.rs` | 5 |
| LED types (Rgb, LedConfig) | `nanod-math/src/led/types.rs` | 7 |
| Button debounce state machine | `nanod-math/src/hmi/button.rs` | 12 |
| RMT WS2811 LED driver (2 strips) | `firmware/src/hmi/leds.rs` | — |
| GPIO button polling (4 buttons) | `firmware/src/hmi/buttons.rs` | — |
| HMI thread (Core 0, buttons + LEDs) | `firmware/src/hmi/hmi_thread.rs` | — |
| Inter-thread channels (mpsc) | `firmware/src/ipc.rs` | — |

### C++ files fully replaced by Phases 1-3A

| C++ File | Status |
|----------|--------|
| `src/haptic.h` / `src/haptic.cpp` | Replaced by `nanod-math` haptic modules |
| `src/haptic_api.h` | Replaced by `haptic/profile.rs` |
| `src/motor.h` | Constants in `motor/foc.rs`, pins in `pins.rs` |
| `include/nanofoc_d.h` | Replaced by `pins.rs` |
| `src/foc_thread.h` / `src/foc_thread.cpp` | Replaced by `thread/foc_thread.rs` |
| `src/thread_crtp.h` | Direct `xTaskCreatePinnedToCore` call |
| `src/com_thread.h` / `src/com_thread.cpp` | Replaced by `com/com_thread.rs` + `com/dispatch.rs` |
| `src/HapticProfileManager.h/cpp` | Replaced by `nanod-math/profile/manager.rs` |
| `src/DeviceSettings.h/cpp` | Partially replaced by `protocol/command.rs` SettingsPayload |
| `src/led_api.h` | Replaced by `nanod-math/led/types.rs` |
| `src/hmi_thread.h/cpp` (buttons + LEDs) | Replaced by `hmi/hmi_thread.rs` + `hmi/buttons.rs` + `hmi/leds.rs` |
| `src/hmi_api.h` (button config) | Partially replaced by `nanod-math/hmi/button.rs` |

---

## What's Left — By Phase

### Phase 2: COM Thread + Serial Protocol — DONE
Completed: JSON serial protocol, profile manager, SPIFFS persistence, command dispatcher.

### Phase 3A: HMI (Buttons + LEDs) — DONE
Completed: GPIO button polling with debounce, RMT WS2811 LED driver (60-LED ring + 8 button LEDs), LED rendering math, HMI thread on Core 0, inter-thread channels (std::sync::mpsc).

### Phase 3B: USB HID/MIDI — NOT STARTED
USB HID (keyboard, mouse, gamepad) and MIDI output. Requires reconfiguring TinyUSB as composite device (CDC+HID+MIDI).

| C++ File | What It Does |
|----------|-------------|
| `src/hmi_thread.h/cpp` (USB parts) | USB HID report sending, MIDI CC, gamepad, knob value mapping |
| `src/hmi_api.h` (HID/MIDI parts) | Key action types (MIDI CC, HID keys, mouse, gamepad, profile change) |

**Deps needed**: TinyUSB FFI composite device setup, HID descriptor tables

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
| Threading | 8 | 6 | **75%** |
| Settings/Profiles | 5 | 4 | **80%** |
| HMI/Input (buttons, LEDs) | 4 | 3 | **75%** |
| USB HID/MIDI | 2 | 0 | 0% |
| Display/UI | 15 | 0 | 0% |
| Audio | 8 | 0 | 0% |
| **Total** | **50** | **21** | **~42%** |

Motor control, serial protocol, profile management, buttons, and LEDs are all ported. Remaining: USB HID/MIDI (Phase 3B), LCD/LVGL (Phase 4), audio/I2S (Phase 5).
