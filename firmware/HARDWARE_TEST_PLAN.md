# NanoD Hardware Test Plan

Test plan for validating the Rust firmware port on physical hardware.
Run these tests after flashing the firmware to the ESP32-S3.

## Prerequisites

- NanoD/Ratchet_H1 device with ESP32-S3
- USB-C cable connected to host
- `nanod` CLI built: `cargo build -p nanod` (from repo root)
- Device flashed: `nanod flash target/xtensa-esp32s3-espidf/release/nanod-firmware`
- Serial monitor available: `nanod monitor` or `nanod test`

## Test Suites

---

### 1. Serial / COM Protocol

Tests the JSON command protocol over USB CDC serial.
Run: `nanod test --suite serial`

| # | Test | Steps | Expected Result |
|---|------|-------|----------------|
| 1.1 | **Get (no profile)** | Send `{"get": true}` before loading any profile | Receive an error or empty message response (no crash) |
| 1.2 | **Upload profile** | Send `{"profile": {"name": "test1", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 20, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}` | Receive `{"msg": {"type": "info", "text": "profile 'test1' set"}}` |
| 1.3 | **Get active profile** | Send `{"get": true}` | Receive profile JSON with `"name": "test1"` and haptic config matching what was sent |
| 1.4 | **Settings roundtrip** | Send `{"settings": {"midi_channel": 7, "led_brightness": 42}}`, then `{"get_settings": true}` | Settings response has `midi_channel: 7` and `led_brightness: 42` |
| 1.5 | **List profiles** | Upload a second profile `{"profile": {"name": "test2"}}`, then send `{"list": true}` | Response contains `"profiles": ["test1", "test2"]` (or superset) |
| 1.6 | **Invalid JSON** | Send `this is not valid json!!!` | Receive an error message (device does not crash or hang) |
| 1.7 | **Save to SPIFFS** | Send `{"save": true}` | Receive `"saving to flash"` confirmation |
| 1.8 | **Load profile** | Send `{"load": "test1"}` | Receive profile JSON for "test1" |
| 1.9 | **Motor recalibrate** | Send `{"motor": {"recalibrate": true}}` | Receive `"recalibrating motor"` confirmation |

---

### 2. Motor / FOC

Tests motor control, encoder, and calibration.
Run: `nanod test --suite motor`

| # | Test | Steps | Expected Result |
|---|------|-------|----------------|
| 2.1 | **Recalibrate** | Send `{"motor": {"recalibrate": true}}` | Device responds with recalibration message; motor may briefly spin during calibration |
| 2.2 | **Encoder events** | Load a profile (60 detents), slowly rotate knob back and forth for 5 seconds | At least 2 `{"angle": {"cur_pos": N}}` events received with changing N |
| 2.3 | **Detent feel** | Rotate the knob slowly | **Manual check:** You should feel distinct haptic detent clicks |

---

### 3. Haptic Detents

Tests haptic feedback profiles and detent behavior.
Run: `nanod test --suite haptic`

| # | Test | Steps | Expected Result |
|---|------|-------|----------------|
| 3.1 | **Default detents (60)** | Upload profile: `{"profile": {"name": "default_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}`. Rotate knob slowly through several detents. | At least 3 angle position events received |
| 3.2 | **Endstop feel** | Upload small-range profile: `{"profile": {"name": "endstop_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 10, "detent_count": 10, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 5.0}}}`. Rotate until you hit the end. | **Manual check:** You feel a firm endstop that prevents further rotation |
| 3.3 | **Vernier mode** | Upload vernier profile: `{"profile": {"name": "vernier_test", "haptic": {"mode": "vernier", "start_pos": 0, "end_pos": 20, "detent_count": 20, "vernier": 5, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}`. Rotate slowly. | **Manual check:** Detents feel finer/closer together than test 3.1 |
| 3.4 | **Profile switch** | Upload profile A (10 detents, wide spacing), feel it. Then upload profile B (60 detents, close spacing), feel it. | **Manual check:** Detent spacing changes noticeably between the two profiles |
| 3.5 | **High detent strength** | Upload with `detent_strength: 8.0` | **Manual check:** Detents feel much stronger/stiffer than 3.0 |
| 3.6 | **Zero detent strength** | Upload with `detent_strength: 0.0` | **Manual check:** Knob spins freely with no detent resistance |

---

### 4. Buttons

Tests the 4 physical buttons (GPIO polling + debounce).
Run: `nanod test --suite buttons`

| # | Test | Steps | Expected Result |
|---|------|-------|----------------|
| 4.1 | **Button A press** | Press and release button A (top) | Receive `{"key": {"num": 0, "state": "pressed"}}` then `{"key": {"num": 0, "state": "released"}}` on serial |
| 4.2 | **Button B press** | Press and release button B | key num=1, pressed then released |
| 4.3 | **Button C press** | Press and release button C | key num=2, pressed then released |
| 4.4 | **Button D press** | Press and release button D | key num=3, pressed then released |
| 4.5 | **Simultaneous press** | Hold button A, then also press button B | Two separate press events, key_state reflects both buttons held |
| 4.6 | **Rapid press** | Tap button A rapidly 10 times | 10 press/release pairs (no missed or doubled events) |
| 4.7 | **No ghost events** | Leave device untouched for 10 seconds | No key events on serial |

---

### 5. LEDs — Ring (60 LEDs)

Tests the 60-LED ring driven via RMT on pin 38.
Run: `nanod test --suite leds`

| # | Test | Steps | Expected Result |
|---|------|-------|----------------|
| 5.1 | **Ring lights up** | Power on device (default profile loaded) | **Visual:** Ring LEDs illuminate with default colors |
| 5.2 | **Pointer tracks knob** | Slowly rotate the knob | **Visual:** A bright pointer LED moves along the ring following your rotation |
| 5.3 | **Halves rendering** | Rotate to midpoint | **Visual:** LEDs before the pointer are one color (primary), LEDs after are another (secondary), pointer is white |
| 5.4 | **Full range** | Rotate from start to end of detent range | **Visual:** Pointer moves from one end of the ring to the other |
| 5.5 | **Custom colors** | Upload profile with LED config: `{"profile": {"name": "color_test", "led": {"brightness": 200, "pointer_col": {"r": 255, "g": 0, "b": 0}, "primary_col": {"r": 0, "g": 255, "b": 0}, "secondary_col": {"r": 0, "g": 0, "b": 255}}}}` | **Visual:** Pointer = red, before = green, after = blue |
| 5.6 | **Brightness control** | Send `{"settings": {"led_brightness": 20}}` | **Visual:** All LEDs become much dimmer |
| 5.7 | **Brightness max** | Send `{"settings": {"led_brightness": 255}}` | **Visual:** All LEDs at full brightness |
| 5.8 | **LEDs disabled** | Upload profile with `"led": {"enabled": false}` | **Visual:** All ring LEDs turn off |
| 5.9 | **Orientation** | Send `{"settings": {"orientation": 1}}`, then `2`, then `3`, then `0` | **Visual:** Ring display rotates 90° each step |

---

### 6. LEDs — Buttons (8 LEDs)

Tests the 8-LED button indicator strip driven via RMT on pin 42.

| # | Test | Steps | Expected Result |
|---|------|-------|----------------|
| 6.1 | **Button LEDs idle** | Don't press any buttons | **Visual:** Each button's LED pair shows its idle color (4 different colors by default) |
| 6.2 | **Button A pressed** | Press and hold button A | **Visual:** Button A's LED pair (LEDs 3,4) switches to white (pressed color) |
| 6.3 | **Button B pressed** | Press and hold button B | **Visual:** Button B's LED pair (LEDs 2,5) switches to white |
| 6.4 | **All buttons pressed** | Hold all 4 buttons | **Visual:** All 8 button LEDs are white |
| 6.5 | **Release restores idle** | Release all buttons | **Visual:** Button LEDs return to their idle colors |
| 6.6 | **Custom button colors** | Upload profile with custom button_colors in LED config | **Visual:** Button idle/pressed colors match the uploaded config |

---

### 7. Inter-Thread Communication

Tests that data flows correctly between FOC, COM, and HMI threads.

| # | Test | Steps | Expected Result |
|---|------|-------|----------------|
| 7.1 | **Profile → haptic** | Upload a profile with haptic config via serial | Detent feel changes immediately (no reboot needed) |
| 7.2 | **Profile → LEDs** | Upload a profile with LED config via serial | Ring colors change immediately |
| 7.3 | **Settings → LEDs** | Send settings with changed brightness | LED brightness changes immediately |
| 7.4 | **Button → serial** | Press a button | Key event appears on serial output within <100ms |
| 7.5 | **Angle → ring** | Rotate knob | Ring pointer updates smoothly (no stutter or lag) |
| 7.6 | **Rapid profile switch** | Alternate between two profiles rapidly (10x in 5 seconds) | No crash, last profile's settings are active |

---

### 8. Stress / Edge Cases

| # | Test | Steps | Expected Result |
|---|------|-------|----------------|
| 8.1 | **Flood serial** | Send 100 JSON commands in rapid succession | Device processes them all (may queue/drop excess), does not crash |
| 8.2 | **Large JSON** | Send a profile with a very long name (200 chars) | Either accepted or graceful error (no crash) |
| 8.3 | **Long uptime** | Leave device running for 30 minutes with occasional knob rotation | No crash, no memory leak (LEDs still responsive) |
| 8.4 | **Power cycle** | Unplug and replug USB | Device boots cleanly, default state restored |
| 8.5 | **Simultaneous knob + buttons** | Rotate knob while pressing buttons | Both angle and key events arrive on serial; LEDs update correctly |

---

### 9. USB HID / MIDI (Phase 3B — when implemented)

These tests are for after TinyUSB composite device is wired up.

| # | Test | Steps | Expected Result |
|---|------|-------|----------------|
| 9.1 | **USB enumeration** | Connect device, check `lsusb` (Linux) or System Report (macOS) | Device shows as composite: CDC + HID + MIDI |
| 9.2 | **CDC serial still works** | Send `{"get": true}` via serial | JSON response received (CDC not broken by adding HID/MIDI) |
| 9.3 | **MIDI CC output** | Configure knob for MIDI CC, rotate knob | Host DAW/MIDI monitor receives CC messages |
| 9.4 | **MIDI channel** | Set `midi_channel: 5`, rotate knob | CC messages arrive on channel 5 |
| 9.5 | **HID keyboard** | Configure button A for keyboard output, press button A | Host receives keypress |
| 9.6 | **HID mouse** | Configure button B for mouse button, press button B | Host registers mouse click |
| 9.7 | **HID gamepad** | Configure button C for gamepad, press button C | Host sees gamepad button press |
| 9.8 | **MIDI + HID simultaneous** | Rotate knob (MIDI CC) while pressing button (keyboard) | Both MIDI and keyboard events arrive on host |

---

## Quick Smoke Test

If you only have 5 minutes, run these:

1. Flash firmware, open `nanod monitor`
2. Send `{"profile": {"name": "smoke", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}` — should get ack
3. Rotate knob — should feel detents and see angle events on serial
4. Send `{"get": true}` — should get profile back
5. Press each button — should see key events on serial
6. Check ring LEDs track the knob pointer
7. Check button LEDs change on press

If all 7 pass, the core firmware is working.

---

## Automated vs Manual

- **Automated** (`nanod test`): serial protocol, motor recalibrate, encoder events, haptic angle events
- **Manual (visual)**: LED colors, ring pointer tracking, brightness, orientation
- **Manual (physical)**: detent feel, endstop feel, vernier feel, button tactile response

The `nanod test` command handles automated tests. Manual tests require you to observe LEDs and feel the haptics.

---

## Known Limitations (current build)

- **No USB HID/MIDI**: Phase 3B deferred — device currently only outputs CDC serial
- **No SPIFFS save/load persistence**: Save command runs but SPIFFS partition may need formatting on first boot
- **No motor recalibration routine**: The `recalibrate` command is acknowledged but the actual calibration sequence is a TODO
- **No display or audio**: Phases 4-5 not started
