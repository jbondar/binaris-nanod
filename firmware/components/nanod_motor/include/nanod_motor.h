// C API for the NanoD motor control subsystem.
// Wraps SimpleFOC + haptic state machine for Rust FFI.
#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

// Initialize the motor, driver, and haptic system.
// pwm_pins: [IN_U, IN_V, IN_W]
// enable_pins: [EN_U, EN_V, EN_W]
// Returns 0 on success.
int nanod_motor_init(
    const int pwm_pins[3],
    const int enable_pins[3],
    int encoder_cs,
    int encoder_sclk,
    int encoder_miso,
    float voltage_supply,
    float voltage_limit,
    float phase_resistance,
    float current_limit
);

// Run motor calibration (align sensor + detect direction).
// Must be called after init. Stores calibration in the motor object.
// Returns 0 on success.
int nanod_motor_calibrate(void);

// Set stored calibration values (from NVS).
// direction: 1=CW, -1=CCW
void nanod_motor_set_calibration(int direction, float zero_electric_angle);

// Get current calibration values.
void nanod_motor_get_calibration(int* direction, float* zero_electric_angle);

// Feed a new encoder angle (multi-turn, radians).
// Call this before nanod_motor_loop().
void nanod_motor_set_encoder_angle(float angle);

// Run one iteration of loopFOC + haptic + move.
// This is the hot-loop function — call at maximum rate.
void nanod_motor_loop(void);

// Get current haptic position (detent index).
uint16_t nanod_motor_get_position(void);

// Get current shaft angle (radians, with offset).
float nanod_motor_get_shaft_angle(void);

// Get current shaft velocity (rad/s).
float nanod_motor_get_shaft_velocity(void);

// Load a haptic profile.
void nanod_motor_load_profile(
    uint8_t mode,        // 0=regular, 1=vernier, 2=viscose, 3=spring
    uint16_t start_pos,
    uint16_t end_pos,
    uint16_t detent_count,
    uint8_t vernier,
    uint8_t kx_force,
    float output_ramp,
    float detent_strength
);

// Trigger recalibration.
void nanod_motor_recalibrate(void);

// Offset the detent system to current shaft position.
void nanod_motor_offset_detent(void);

#ifdef __cplusplus
}
#endif
