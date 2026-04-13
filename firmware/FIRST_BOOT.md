# NanoD Firmware — First Boot Guide

How to build, flash, and bring up the Rust firmware on a NanoD device for the first time.

## 1. Build the Firmware

```bash
cd firmware

# Debug build (faster compile, larger binary, has debug logging)
cargo build

# Release build (slower compile, smaller binary, optimized)
cargo build --release
```

Build output:
- **ELF**: `target/xtensa-esp32s3-espidf/debug/nanod-firmware` (or `release/`)
- **Bootloader**: `target/xtensa-esp32s3-espidf/debug/bootloader.bin`
- **Partition table**: `target/xtensa-esp32s3-espidf/debug/partition-table.bin`

## 2. Install the Flash Tool

You need `espflash` to flash ELF files to the ESP32-S3. Install it once:

```bash
cargo install espflash
```

Verify:
```bash
espflash --version
# espflash 3.x.x
```

## 3. Connect the Device

1. Plug the NanoD into your Mac via USB-C
2. The device should enumerate as a USB serial device

Check it's visible:
```bash
ls /dev/cu.usb*
# Should show something like /dev/cu.usbmodem14101
```

If the device doesn't show up, it may need to be put into bootloader mode:
- Hold the **BOOT** button while pressing **RESET** (or while plugging in USB)
- The device should now appear as a serial port

## 4. Flash the Firmware (First Time)

First-time flash writes the bootloader, partition table, and application:

```bash
cd firmware

# Flash debug build (includes bootloader + partition table automatically)
espflash flash target/xtensa-esp32s3-espidf/debug/nanod-firmware --monitor

# Or flash release build
espflash flash target/xtensa-esp32s3-espidf/release/nanod-firmware --monitor
```

`espflash flash` handles:
- Converting ELF → ESP32 binary image format
- Writing bootloader to 0x0
- Writing partition table to 0x8000
- Writing app to 0x10000
- Resetting the device
- `--monitor` opens serial monitor after flash

If espflash can't find the port automatically:
```bash
espflash flash target/xtensa-esp32s3-espidf/debug/nanod-firmware --port /dev/cu.usbmodem14101 --monitor
```

### Alternative: `cargo run`

The firmware's `.cargo/config.toml` has `runner = "espflash flash --monitor"`, so you can also just:

```bash
cd firmware
cargo run          # debug build + flash + monitor
cargo run --release  # release build + flash + monitor
```

## 5. Verify Boot

After flashing, the serial monitor should show boot logs:

```
I (xxx) NanoD firmware starting
I (xxx) FOC thread starting
I (xxx) FOC thread initialized, entering control loop
I (xxx) COM thread starting
I (xxx) COM thread ready, listening for JSON commands
I (xxx) HMI thread starting
I (xxx) HMI thread initialized, entering main loop
I (xxx) All threads spawned
```

Key things to check:
- **FOC thread**: initializes SPI (encoder), MCPWM (motor driver), loads calibration from NVS
- **COM thread**: ready for JSON commands on USB CDC serial
- **HMI thread**: initializes button GPIOs and RMT LED driver

### If FOC thread fails

If you see `FOC task failed`, the most likely causes are:
- Encoder SPI wiring issue (check MT6701 connections on pins 17, 18, 21)
- MCPWM driver init failure (check motor driver connections on pins 33-37)
- No NVS calibration yet (warning is normal on first boot)

The COM and HMI threads should still start even if FOC fails.

## 6. First-Time Calibration

The motor needs calibration on first boot (direction detection + electrical zero angle).

```
>>> {"motor": {"recalibrate": true}}
```

**Note:** The full calibration routine is currently a TODO stub. It acknowledges the command but doesn't run the actual calibration sequence yet. This will be implemented when we have the device for testing.

For now, the motor will use default calibration values. Haptic detents may not feel correct until proper calibration is done.

## 7. Quick Sanity Check

Once booted, test basic functionality. Open a second terminal:

```bash
# Build the CLI tool (if not already built)
cd /path/to/NanoD-Integrations
cargo build -p nanod --release

# Open monitor (if not already open from espflash)
nanod monitor
```

Then paste these commands into the monitor:

```
>>> {"profile": {"name": "test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 1, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}
```
Expected: `{"msg":{"type":"info","text":"profile 'test' set"}}`

```
>>> {"get": true}
```
Expected: profile JSON echoed back.

Try rotating the knob — you should see angle events:
```
<<< {"angle":{"cur_pos":42}}
```

Try pressing buttons — you should see key events:
```
<<< {"key":{"num":0,"state":"pressed"}}
```

Check LEDs:
- Ring LEDs should illuminate and track the knob position
- Button LEDs should change color when pressed

If all of that works, move on to the full [Hardware Test Plan](HARDWARE_TEST_PLAN.md).

## 8. Subsequent Flashes

After the first flash, you don't need to reflash the bootloader/partition table. Just:

```bash
cd firmware
cargo run          # build + flash + monitor
```

Or if you want to flash without monitoring:

```bash
espflash flash target/xtensa-esp32s3-espidf/debug/nanod-firmware
```

## 9. Recovery

If the device gets into a bad state (bootloop, bricked firmware):

1. Put device into bootloader mode: hold **BOOT** + press **RESET** (or hold BOOT while plugging in USB)
2. Flash the working firmware:
   ```bash
   espflash flash target/xtensa-esp32s3-espidf/debug/nanod-firmware --port /dev/cu.usbmodem14101
   ```
3. Or use the nanod recovery command:
   ```bash
   nanod recover target/xtensa-esp32s3-espidf/debug/nanod-firmware
   ```

## 10. Erase Flash (Nuclear Option)

If you need to completely wipe the device (removes NVS calibration, SPIFFS profiles, everything):

```bash
espflash erase-flash --port /dev/cu.usbmodem14101
```

Then reflash from step 4.

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| No serial port visible | Device not in bootloader mode | Hold BOOT + RESET, or hold BOOT while plugging in |
| `espflash` can't connect | Wrong port or baud | Try `--port /dev/cu.usbmodemXXXX`, check `ls /dev/cu.usb*` |
| `Permission denied` on port | User not in dialout group (Linux) | `sudo usermod -a -G dialout $USER` then re-login |
| Build fails: `can't find crate for core` | Wrong Rust toolchain | Check `firmware/rust-toolchain.toml` has `channel = "esp"`, run `espup install` |
| Build fails: Python/CMake error | ESP-IDF build cache issue | Delete `firmware/target/` and rebuild |
| FOC thread fails on boot | Hardware wiring issue | Check encoder SPI + motor MCPWM pin connections |
| No angle events | FOC thread not running | Check boot logs for FOC errors |
| No key events | HMI thread not running | Check boot logs for HMI errors |
| LEDs don't light up | RMT driver init failed | Check boot logs; verify LED_A (pin 38) and LED_B (pin 42) wiring |
