# NanoD Hardware Test Plan

Test plan for validating the Rust firmware port on physical hardware.
Run these tests after flashing the firmware to the ESP32-S3.

## Prerequisites

If this is your first time flashing, follow the [First Boot Guide](FIRST_BOOT.md) first.

```bash
# Build the CLI tool (from repo root)
cargo build -p nanod --release

# Build and flash the firmware (from firmware/ dir)
cd firmware
cargo run --release    # builds + flashes + opens monitor

# Or flash separately
espflash flash target/xtensa-esp32s3-espidf/release/nanod-firmware

# Open serial monitor (use this for all manual send/receive tests)
nanod monitor
```

Everything below assumes `nanod monitor` is open. Lines starting with `>>>` are what you type/paste into the monitor. Lines starting with `<<<` are expected responses.

---

## 1. Serial / COM Protocol

**Automated:** `nanod test --suite serial`

Or run manually:

### 1.1 Get (no profile)
```
>>> {"get": true}
<<< {"msg":{"type":"error","text":"no active profile"}}
```

### 1.2 Upload profile
```
>>> {"profile": {"name": "test1", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 20, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
<<< {"msg":{"type":"info","text":"profile 'test1' set"}}
```

### 1.3 Get active profile
```
>>> {"get": true}
<<< {"profile":{"name":"test1","haptic":{"mode":"regular","start_pos":0,"end_pos":100,...}}}
```
Verify: name is "test1", haptic config matches what was sent.

### 1.4 Settings roundtrip
```
>>> {"settings": {"midi_channel": 7, "led_brightness": 42}}
<<< {"msg":{"type":"info","text":"settings updated"}}

>>> {"get_settings": true}
<<< {"settings":{"midi_channel":7,...,"led_brightness":42,...}}
```
Verify: midi_channel=7, led_brightness=42.

### 1.5 List profiles
```
>>> {"profile": {"name": "test2"}}
<<< {"msg":{"type":"info","text":"profile 'test2' set"}}

>>> {"list": true}
<<< {"profiles":["test1","test2"]}
```

### 1.6 Invalid JSON
```
>>> this is not valid json!!!
<<< {"msg":{"type":"error","text":"..."}}
```
Verify: error response, device does not crash or hang.

### 1.7 Save to SPIFFS
```
>>> {"save": true}
<<< {"msg":{"type":"info","text":"saving to flash"}}
```

### 1.8 Load profile
```
>>> {"load": "test1"}
<<< {"profile":{"name":"test1",...}}
```

### 1.9 Motor recalibrate command
```
>>> {"motor": {"recalibrate": true}}
<<< {"msg":{"type":"info","text":"recalibrating motor"}}
```

---

## 2. Motor / FOC

**Automated:** `nanod test --suite motor`

### 2.1 Recalibrate
```
>>> {"motor": {"recalibrate": true}}
<<< {"msg":{"type":"info","text":"recalibrating motor"}}
```
Note: motor may briefly spin during calibration.

### 2.2 Encoder events
```
>>> {"profile": {"name": "enc_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
```
Then slowly rotate knob back and forth for 5 seconds.
```
<<< {"angle":{"cur_pos":42}}
<<< {"angle":{"cur_pos":43}}
<<< {"angle":{"cur_pos":44}}
...
```
Verify: at least 2 angle events with changing `cur_pos`.

### 2.3 Detent feel
Rotate knob slowly.
**Manual check:** you should feel distinct haptic detent clicks.

---

## 3. Haptic Detents

**Automated:** `nanod test --suite haptic`

### 3.1 Default detents (60)
```
>>> {"profile": {"name": "default_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
```
Rotate knob slowly through several detents.
Verify: at least 3 angle events received. Detents feel evenly spaced.

### 3.2 Endstop feel
```
>>> {"profile": {"name": "endstop_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 10, "detent_count": 10, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 5.0}}}
```
Rotate until you hit the end.
**Manual check:** you feel a firm endstop that prevents further rotation.

### 3.3 Vernier mode
```
>>> {"profile": {"name": "vernier_test", "haptic": {"mode": "vernier", "start_pos": 0, "end_pos": 20, "detent_count": 20, "vernier": 5, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
```
Rotate slowly.
**Manual check:** detents feel finer/closer together than test 3.1 (20 coarse × 5 vernier = 100 fine steps).

### 3.4 Profile switch
```
>>> {"profile": {"name": "switch_A", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 10, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
```
Feel the wide detent spacing. Then:
```
>>> {"profile": {"name": "switch_B", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
```
**Manual check:** detent spacing changes noticeably (much closer together).

### 3.5 High detent strength
```
>>> {"profile": {"name": "strong", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 30, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 8.0}}}
```
**Manual check:** detents feel much stiffer than strength 3.0.

### 3.6 Zero detent strength
```
>>> {"profile": {"name": "free", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 30, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 0.0}}}
```
**Manual check:** knob spins freely with no detent resistance.

---

## 4. Buttons

**Automated:** `nanod test --suite buttons`

For manual testing, open `nanod monitor` and watch for key events.

### 4.1 Button A
Press and release button A (top).
```
<<< {"key":{"num":0,"state":"pressed"}}
<<< {"key":{"num":0,"state":"released"}}
```

### 4.2 Button B
Press and release button B.
```
<<< {"key":{"num":1,"state":"pressed"}}
<<< {"key":{"num":1,"state":"released"}}
```

### 4.3 Button C
Press and release button C.
```
<<< {"key":{"num":2,"state":"pressed"}}
<<< {"key":{"num":2,"state":"released"}}
```

### 4.4 Button D
Press and release button D.
```
<<< {"key":{"num":3,"state":"pressed"}}
<<< {"key":{"num":3,"state":"released"}}
```

### 4.5 Simultaneous press
Hold button A, then also press button B.
```
<<< {"key":{"num":0,"state":"pressed"}}
<<< {"key":{"num":1,"state":"pressed"}}
```
Verify: both events arrive, no missed presses.

### 4.6 Rapid press
Tap button A rapidly 10 times.
Verify: 10 pressed + 10 released events (no missed or doubled).

### 4.7 No ghost events
Leave device untouched for 10 seconds.
Verify: no key events appear on serial.

---

## 5. LEDs — Ring (60 LEDs)

### 5.1 Ring lights up
Power on device. Load a profile:
```
>>> {"profile": {"name": "led_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
```
**Visual:** ring LEDs illuminate with default colors (teal primary, dark red secondary, white pointer).

### 5.2 Pointer tracks knob
Slowly rotate the knob.
**Visual:** a bright white pointer LED moves along the ring following your rotation.

### 5.3 Halves rendering
Rotate to roughly the midpoint.
**Visual:** LEDs before the pointer are teal (primary), LEDs after are dark red (secondary), pointer is white.

### 5.4 Full range
Rotate from start to end of detent range.
**Visual:** pointer moves from one end of the ring to the other.

### 5.5 Custom colors (red/green/blue)
```
>>> {"profile": {"name": "rgb_test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}, "led": {"enabled": true, "brightness": 200, "pointer_col": {"r": 255, "g": 0, "b": 0}, "primary_col": {"r": 0, "g": 255, "b": 0}, "secondary_col": {"r": 0, "g": 0, "b": 255}, "button_colors": [{"idle": {"r": 255, "g": 0, "b": 0}, "pressed": {"r": 255, "g": 255, "b": 255}}, {"idle": {"r": 0, "g": 255, "b": 0}, "pressed": {"r": 255, "g": 255, "b": 255}}, {"idle": {"r": 0, "g": 0, "b": 255}, "pressed": {"r": 255, "g": 255, "b": 255}}, {"idle": {"r": 255, "g": 255, "b": 0}, "pressed": {"r": 255, "g": 255, "b": 255}}]}}}
```
**Visual:** pointer = RED, before pointer = GREEN, after pointer = BLUE.

### 5.6 Brightness low
```
>>> {"settings": {"led_brightness": 20}}
```
**Visual:** all LEDs become much dimmer.

### 5.7 Brightness max
```
>>> {"settings": {"led_brightness": 255}}
```
**Visual:** all LEDs at full brightness.

### 5.8 LEDs disabled
```
>>> {"profile": {"name": "dark", "led": {"enabled": false}}}
```
**Visual:** all ring LEDs turn off.

### 5.9 LEDs re-enabled
```
>>> {"profile": {"name": "bright", "led": {"enabled": true, "brightness": 150}}}
```
**Visual:** ring LEDs turn back on.

### 5.10 Orientation rotation
```
>>> {"settings": {"orientation": 0}}
```
Note pointer position. Then:
```
>>> {"settings": {"orientation": 1}}
```
**Visual:** entire ring display rotates 90° clockwise.
```
>>> {"settings": {"orientation": 2}}
```
**Visual:** 180° from original.
```
>>> {"settings": {"orientation": 3}}
```
**Visual:** 270° from original.
```
>>> {"settings": {"orientation": 0}}
```
**Visual:** back to original position.

---

## 6. LEDs — Buttons (8 LEDs)

### 6.1 Button LEDs idle
Don't press any buttons.
**Visual:** each button's LED pair shows a distinct idle color (defaults: teal, navy, purple, dark red).

### 6.2 Button A press
Press and hold button A.
**Visual:** button A's LED pair switches to white.

### 6.3 Button B press
Press and hold button B.
**Visual:** button B's LED pair switches to white.

### 6.4 All buttons pressed
Hold all 4 buttons simultaneously.
**Visual:** all 8 button LEDs are white.

### 6.5 Release restores idle
Release all buttons.
**Visual:** button LEDs return to their idle colors.

### 6.6 Custom button colors
Use the profile from test 5.5 (RGB test). Button idle colors should be:
- Button A: red
- Button B: green
- Button C: blue
- Button D: yellow

Press each button — should switch to white.

---

## 7. Inter-Thread Communication

### 7.1 Profile → haptic (instant)
```
>>> {"profile": {"name": "ipc1", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 10, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 5.0}}}
```
Verify: detent feel changes immediately without rebooting.

### 7.2 Profile → LEDs (instant)
```
>>> {"profile": {"name": "ipc2", "led": {"brightness": 200, "pointer_col": {"r": 0, "g": 255, "b": 0}, "primary_col": {"r": 255, "g": 0, "b": 0}, "secondary_col": {"r": 0, "g": 0, "b": 255}}}}
```
**Visual:** ring colors change immediately.

### 7.3 Settings → brightness (instant)
```
>>> {"settings": {"led_brightness": 30}}
```
**Visual:** brightness drops immediately.
```
>>> {"settings": {"led_brightness": 200}}
```
**Visual:** brightness restores immediately.

### 7.4 Button → serial (<100ms)
Press button A while watching serial output.
Verify: `{"key":{"num":0,"state":"pressed"}}` appears with no perceptible delay.

### 7.5 Angle → ring (smooth)
Rotate knob at various speeds.
**Visual:** ring pointer tracks smoothly with no stutter or visible lag.

### 7.6 Rapid profile switch
Paste these 10 commands rapidly:
```
>>> {"profile": {"name": "A", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 10, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
>>> {"profile": {"name": "B", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
>>> {"profile": {"name": "A", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 10, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
>>> {"profile": {"name": "B", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
>>> {"profile": {"name": "A", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 100, "detent_count": 10, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
```
Verify: no crash, last profile's detent count is active (10 in this case).

---

## 8. Stress / Edge Cases

### 8.1 Flood serial
Send `{"get_settings": true}` 100 times in rapid succession (script or paste).
Verify: device responds to all (may be slow), does not crash.

### 8.2 Large profile name
```
>>> {"profile": {"name": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}
```
Verify: either accepted or graceful error (no crash).

### 8.3 Long uptime
Leave device powered for 30+ minutes. Occasionally rotate knob and press buttons.
Verify: no crash, LEDs still responsive, events still arrive on serial.

### 8.4 Power cycle
Unplug USB cable, wait 3 seconds, replug.
Verify: device boots cleanly, `nanod monitor` can reconnect, serial protocol works.

### 8.5 Simultaneous knob + buttons
Rotate knob continuously while pressing and releasing buttons.
Verify: both angle events and key events arrive on serial, LEDs update for both.

---

## 9. USB HID / MIDI (Phase 3B — not yet implemented)

These tests are placeholders for when TinyUSB composite is wired up.

### 9.1 USB enumeration
```bash
# macOS
system_profiler SPUSBDataType | grep -A5 "Nano_D"

# Linux
lsusb | grep "Nano_D"
```
Expected: device shows as composite with CDC + HID + MIDI interfaces.

### 9.2 CDC still works after composite
```
>>> {"get": true}
```
Verify: JSON response received (adding HID/MIDI didn't break CDC serial).

### 9.3 MIDI CC output
Configure knob for MIDI CC, rotate knob.
```bash
# macOS — open Audio MIDI Setup, verify MIDI device appears
# Use a MIDI monitor app to see CC messages

# Linux
amidi -l                    # list MIDI devices
aseqdump -p <port>          # monitor MIDI events
```
Verify: CC messages arrive as you rotate.

### 9.4-9.8
(Deferred until TinyUSB composite is implemented)

---

## Quick Smoke Test (5 minutes)

Run this sequence to validate the core firmware:

```bash
# 1. Flash and open monitor
nanod flash firmware.bin
nanod monitor
```

```
# 2. Upload a profile
>>> {"profile": {"name": "smoke", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
<<< {"msg":{"type":"info","text":"profile 'smoke' set"}}

# 3. Rotate knob — feel detents, watch for angle events
<<< {"angle":{"cur_pos":...}}

# 4. Read back profile
>>> {"get": true}
<<< {"profile":{"name":"smoke",...}}

# 5. Press each button — watch for key events
<<< {"key":{"num":0,"state":"pressed"}}
<<< {"key":{"num":0,"state":"released"}}

# 6. Visual: ring LEDs track knob pointer
# 7. Visual: button LEDs change color on press
```

**Pass criteria:** All 7 checks above work. If any fail, check logs with `nanod monitor --baud 115200` for error output.

---

## Automated Test Runner

For the automated suites, use the `nanod test` command:

```bash
# Run all automated suites
nanod test all

# Run individual suites
nanod test serial
nanod test motor
nanod test haptic
nanod test buttons
nanod test leds

# Specify port if not auto-detected
nanod test all --port /dev/ttyACM0
```

The automated runner sends JSON commands, waits for responses, and reports pass/fail. Physical checks (detent feel, LED visuals) prompt you with y/n questions.

---

## Known Limitations (current build)

- **No USB HID/MIDI** — Phase 3B deferred. Device only outputs CDC serial.
- **No SPIFFS persistence across reboots** — save command runs but partition may need formatting on first boot.
- **Motor recalibration is a stub** — command is acknowledged but full calibration sequence is a TODO.
- **No display or audio** — Phases 4-5 not started.
- **Angle events may not arrive** — the FOC→HMI→COM channel depends on all three threads running; if FOC thread fails to init (encoder/driver issue), no angle events will appear.
