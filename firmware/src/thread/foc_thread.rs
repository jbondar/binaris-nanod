use core::f32::consts::PI;

use esp_idf_hal::gpio::{AnyInputPin, PinDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::spi::{SpiDriver, SpiDriverConfig};
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvsPartition};
use esp_idf_sys::*;

use crate::haptic::profile::{Direction, MotorCalibration};
use crate::haptic::state::HapticController;
use crate::ipc::{AngleSnapshot, FocCommand, FocContext};
use crate::motor::calibration;
use crate::motor::driver::ThreePhaseDriver;
use crate::motor::encoder::Mt6701Encoder;
use crate::motor::foc::{set_phase_voltage, FocState, MotorConfig};
use crate::pins;

const FOC_STACK_SIZE: usize = 16384;
const FOC_PRIORITY: u32 = 1;
const FOC_CORE: i32 = 1;
const PWM_FREQUENCY_HZ: u32 = 25_000;

/// How often to publish angle snapshots (microseconds).
const ANGLE_PUBLISH_INTERVAL_US: u64 = 10_000; // 10ms

/// Voltage used during calibration alignment (lower = gentler, higher = more reliable).
const CALIBRATION_VOLTAGE: f32 = 3.5;

/// Spawn the FOC control thread pinned to Core 1.
pub fn spawn_foc_thread(ctx: FocContext) {
    let ctx_ptr = Box::into_raw(Box::new(ctx)) as *mut core::ffi::c_void;
    unsafe {
        let mut handle: TaskHandle_t = core::ptr::null_mut();
        xTaskCreatePinnedToCore(
            Some(foc_task),
            b"foc\0".as_ptr() as *const _,
            FOC_STACK_SIZE as u32,
            ctx_ptr,
            FOC_PRIORITY,
            &mut handle,
            FOC_CORE,
        );
    }
}

unsafe extern "C" fn foc_task(arg: *mut core::ffi::c_void) {
    let ctx = unsafe { *Box::from_raw(arg as *mut FocContext) };
    if let Err(e) = foc_task_inner(ctx) {
        log::error!("FOC task failed: {:?}", e);
    }
    loop {
        vTaskDelay(1000);
    }
}

/// Run the motor calibration routine.
///
/// 1. Apply voltage at electrical angle 0 → motor aligns to known position
/// 2. Read encoder → that's the zero electrical angle offset
/// 3. Apply voltage at a positive offset → motor moves
/// 4. Read encoder again → determine direction (CW or CCW)
/// 5. Store calibration to NVS
fn run_calibration(
    driver: &mut ThreePhaseDriver,
    encoder: &mut Mt6701Encoder,
    foc: &mut FocState,
    nvs_partition: EspNvsPartition<esp_idf_svc::nvs::NvsDefault>,
) -> Result<MotorCalibration, EspError> {
    log::info!("Calibration: starting — applying alignment voltage");

    let voltage_limit = foc.config.voltage_limit;
    let pole_pairs = foc.config.pole_pairs as f32;

    // Step 1: Apply voltage at electrical angle 0 to align rotor
    let duty = set_phase_voltage(CALIBRATION_VOLTAGE, 0.0, 0.0, voltage_limit);
    driver.set_pwm(duty)?;

    // Wait for motor to settle at alignment position
    delay_ms(1000);

    // Step 2: Read encoder angle at alignment (this is the zero reference)
    // Take multiple readings and average for stability
    let mut sum = 0.0;
    for _ in 0..10 {
        sum += encoder.read_angle()?;
        delay_ms(5);
    }
    let angle_at_zero = sum / 10.0;
    log::info!(
        "Calibration: aligned at electrical angle 0, encoder reads {:.4} rad",
        angle_at_zero
    );

    // Step 3: Sweep through several electrical angles to detect direction
    // Move a full electrical revolution (2*PI) in steps
    let steps = 20;
    let step_angle = 2.0 * PI / steps as f32;
    for i in 1..=steps {
        let elec_angle = step_angle * i as f32;
        let duty = set_phase_voltage(CALIBRATION_VOLTAGE, 0.0, elec_angle, voltage_limit);
        driver.set_pwm(duty)?;
        delay_ms(50);
    }

    // Hold at 2*PI (full electrical revolution) and wait for settle
    let duty = set_phase_voltage(CALIBRATION_VOLTAGE, 0.0, 2.0 * PI, voltage_limit);
    driver.set_pwm(duty)?;
    delay_ms(500);

    // Step 4: Read encoder at new position (average for stability)
    let mut sum2 = 0.0;
    for _ in 0..10 {
        sum2 += encoder.read_angle()?;
        delay_ms(5);
    }
    let angle_after_move = sum2 / 10.0;
    let delta = angle_after_move - angle_at_zero;

    log::info!(
        "Calibration: moved to +90 elec deg, encoder delta = {:.4} rad",
        delta
    );

    // Determine direction: if positive electrical angle caused positive shaft movement → CW
    let direction = if delta > 0.0 {
        Direction::Cw
    } else {
        Direction::Ccw
    };

    let sensor_dir: i8 = match direction {
        Direction::Cw => 1,
        Direction::Ccw => -1,
        Direction::Unknown => 1,
    };

    // Calculate zero electrical angle
    // electrical_angle = sensor_direction * shaft_angle * pole_pairs - zero_electrical_angle
    // At alignment (electrical angle = 0): 0 = sensor_dir * angle_at_zero * pole_pairs - zero
    // Therefore: zero = sensor_dir * angle_at_zero * pole_pairs
    let zero_electrical_angle =
        nanod_math::motor::foc::normalize_angle(sensor_dir as f32 * angle_at_zero * pole_pairs);

    log::info!(
        "Calibration: direction={:?}, zero_angle={:.4}",
        direction,
        zero_electrical_angle
    );

    // Step 5: Stop applying voltage
    driver.set_pwm(nanod_math::motor::foc::PhaseDuty::default())?;

    // Apply calibration to FOC state
    foc.sensor_direction = sensor_dir;
    foc.zero_electrical_angle = zero_electrical_angle;

    let cal = MotorCalibration {
        direction,
        zero_angle: zero_electrical_angle,
    };

    // Store to NVS
    calibration::store_calibration(nvs_partition, &cal)?;
    log::info!("Calibration: stored to NVS");

    Ok(cal)
}

fn delay_ms(ms: u32) {
    // At 100Hz tick rate, 1 tick = 10ms. Use esp_timer for more precise delays.
    let us = ms as u64 * 1000;
    let start = unsafe { esp_timer_get_time() } as u64;
    while (unsafe { esp_timer_get_time() } as u64) - start < us {
        // Busy wait for calibration timing accuracy
    }
}

fn foc_task_inner(ctx: FocContext) -> Result<(), EspError> {
    log::info!("FOC thread starting");

    let peripherals = unsafe { Peripherals::steal() };
    let nvs_partition = EspDefaultNvsPartition::take()?;

    // --- SPI bus for encoder ---
    // MT6701 SSI: CLK=18, sensor DO=21 → ESP32 MISO. No MOSI needed.
    // SpiDriver requires an sdo (MOSI) pin — use gpio4 (display MOSI, unused)
    // as a dummy since SSI is read-only.
    let spi_driver = SpiDriver::new(
        peripherals.spi2,
        peripherals.pins.gpio18, // SCLK
        peripherals.pins.gpio4,  // SDO (MOSI) — dummy, not connected to sensor
        Some(peripherals.pins.gpio21), // SDI (MISO) — MT6701 DO pin
        &SpiDriverConfig::new(),
    )?;

    let mut encoder = Mt6701Encoder::new(
        &spi_driver,
        peripherals.pins.gpio17.into(), // CS
    )?;

    // --- Motor driver (MCPWM) ---
    let en_u = PinDriver::output(peripherals.pins.gpio33)?;
    let en_v = PinDriver::output(peripherals.pins.gpio48)?;
    let en_w = PinDriver::output(peripherals.pins.gpio36)?;

    let mut driver = ThreePhaseDriver::new(
        0,
        [pins::MOTOR_IN_U, pins::MOTOR_IN_V, pins::MOTOR_IN_W],
        [en_u, en_v, en_w],
        PWM_FREQUENCY_HZ,
    )?;
    driver.enable()?;

    // --- FOC state ---
    let mut foc = FocState::new(MotorConfig::default());

    // --- Load or run calibration ---
    let cal = calibration::load_calibration(nvs_partition.clone())?;
    if cal.direction != Direction::Unknown {
        foc.sensor_direction = match cal.direction {
            Direction::Cw => 1,
            Direction::Ccw => -1,
            Direction::Unknown => 1,
        };
        foc.zero_electrical_angle = cal.zero_angle;
        log::info!(
            "Loaded calibration: dir={}, zero={}",
            foc.sensor_direction,
            foc.zero_electrical_angle
        );
    } else {
        log::warn!("No calibration found — running auto-calibration");
        match run_calibration(&mut driver, &mut encoder, &mut foc, nvs_partition.clone()) {
            Ok(cal) => log::info!("Auto-calibration complete: {:?}", cal.direction),
            Err(e) => log::error!("Auto-calibration failed: {:?}", e),
        }
    }

    // --- Haptic controller with default profile ---
    let mut haptic = HapticController::new();
    haptic.sensor_direction = match foc.sensor_direction {
        1 => Direction::Cw,
        -1 => Direction::Ccw,
        _ => Direction::Unknown,
    };

    // Load a default detent profile so the knob has haptics on startup
    use crate::haptic::profile::{DetentProfile, HapticMode};
    let default_profile = DetentProfile {
        mode: HapticMode::Regular,
        start_pos: 0,
        end_pos: 255,
        detent_count: 60,
        vernier: 1,
        kx_force: false,
        output_ramp: 5000.0,
        detent_strength: 3.0,
    };
    haptic.state.load_profile(default_profile, None);
    log::info!("FOC: loaded default haptic profile (60 detents, 0-255)");

    // Initialize attract_angle to current shaft position so we don't snap on startup
    let initial_angle = encoder.read_angle()?;
    foc.update_sensor(initial_angle, unsafe { esp_timer_get_time() } as u64);
    haptic.state.attract_angle =
        (foc.shaft_angle / haptic.state.detent_width).round() * haptic.state.detent_width;
    haptic.state.last_attract_angle = haptic.state.attract_angle;
    // Set current_pos to match
    let initial_pos = (haptic.state.attract_angle / haptic.state.detent_width).round() as u16;
    haptic.state.current_pos = initial_pos;
    haptic.state.last_pos = initial_pos;
    log::info!(
        "FOC: initial angle={:.3}, attract={:.3}, pos={}",
        foc.shaft_angle,
        haptic.state.attract_angle,
        haptic.state.current_pos
    );

    log::info!("FOC thread initialized, entering control loop");

    let mut last_publish_us: u64 = 0;

    // --- Main control loop ---
    loop {
        let now_us = unsafe { esp_timer_get_time() } as u64;

        // Check for incoming commands (non-blocking)
        if let Ok(cmd) = ctx.cmd_rx.try_recv() {
            match cmd {
                FocCommand::UpdateHaptic(profile) => {
                    log::info!(
                        "FOC: haptic update — {} detents, {:?} mode",
                        profile.detent_count,
                        profile.mode
                    );
                    haptic.state.load_profile(profile, None);
                }
                FocCommand::Recalibrate => {
                    log::info!("FOC: recalibration requested");
                    driver.disable()?;
                    delay_ms(500);
                    foc.sensor_direction = 1;
                    foc.zero_electrical_angle = 0.0;
                    driver.enable()?;
                    match run_calibration(
                        &mut driver,
                        &mut encoder,
                        &mut foc,
                        nvs_partition.clone(),
                    ) {
                        Ok(cal) => {
                            haptic.sensor_direction = cal.direction;
                            log::info!("Recalibration complete: {:?}", cal.direction);
                        }
                        Err(e) => log::error!("Recalibration failed: {:?}", e),
                    }
                }
            }
        }

        let angle = encoder.read_angle()?;
        foc.update_sensor(angle, now_us);

        let output = haptic.haptic_loop(foc.shaft_angle, foc.shaft_velocity, now_us);

        if output.run_foc {
            let duty = foc.compute_torque(output.pid_error);
            driver.set_pwm(duty)?;
        } else {
            loop {
                let settle_now = unsafe { esp_timer_get_time() } as u64;
                let settle_angle = encoder.read_angle()?;
                foc.update_sensor(settle_angle, settle_now);

                let (error, should_break) =
                    haptic.bounds_settle_error(foc.shaft_angle, foc.shaft_velocity);

                let duty = foc.compute_torque(haptic.pid.call(error, settle_now));
                driver.set_pwm(duty)?;

                if should_break {
                    break;
                }
            }
        }

        // Publish angle snapshot to HMI (throttled) + debug logging
        if now_us.wrapping_sub(last_publish_us) >= ANGLE_PUBLISH_INTERVAL_US {
            last_publish_us = now_us;
            let _ = ctx.angle_tx.try_send(AngleSnapshot {
                shaft_angle: foc.shaft_angle,
                current_pos: haptic.state.current_pos as u16,
            });

            // Debug: log FOC state periodically
            if (now_us / 1_000_000) % 2 == 0 && (now_us / 10_000) % 50 == 0 {
                let raw = encoder.get_raw_angle().unwrap_or(-1.0);
                let duty = foc.compute_torque(output.pid_error);
                log::info!(
                    "FOC: raw={:.3} angle={:.3} vel={:.1} pos={} err={:.3} duty=({:.2},{:.2},{:.2})",
                    raw,
                    foc.shaft_angle,
                    foc.shaft_velocity,
                    haptic.state.current_pos,
                    output.pid_error,
                    duty.a, duty.b, duty.c,
                );
            }
        }

        // No delay — FOC runs at max speed for tight motor control.
        // Watchdog for Core 1 IDLE is disabled via sdkconfig.
    }
}
