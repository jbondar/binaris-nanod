// NanoD motor control — C API wrapping SimpleFOC + haptic.
// This is the bridge between Rust and the C++ motor control stack.

#include "include/nanod_motor.h"
#include "arduino_shim.h"
#include "simplefoc/BLDCMotor.h"
#include "simplefoc/BLDCDriver3PWM.h"
#include "ext_sensor.h"
#include "haptic.h"

// Global instances (single motor system)
static BLDCMotor* motor = nullptr;
static BLDCDriver3PWM* driver = nullptr;
static ExternalSensor* sensor = nullptr;
static HapticInterface* haptic = nullptr;
static PIDController* haptic_pid = nullptr;

extern "C" {

int nanod_motor_init(
    const int pwm_pins[3],
    const int enable_pins[3],
    float voltage_supply,
    float voltage_limit,
    float phase_resistance,
    float current_limit
) {
    // Create driver
    driver = new BLDCDriver3PWM(pwm_pins[0], pwm_pins[1], pwm_pins[2],
                                 enable_pins[0], enable_pins[1], enable_pins[2]);
    driver->voltage_power_supply = voltage_supply;
    driver->voltage_limit = voltage_limit;
    if (driver->init() == 0) return -1;

    // Create sensor
    sensor = new ExternalSensor();
    sensor->init();

    // Create motor
    motor = new BLDCMotor(7, phase_resistance); // 7 pole pairs
    motor->linkSensor(sensor);
    motor->linkDriver(driver);
    motor->voltage_limit = voltage_limit;
    motor->current_limit = current_limit;
    motor->LPF_velocity.Tf = 0.01f;
    motor->controller = MotionControlType::torque;
    motor->foc_modulation = FOCModulationType::SpaceVectorPWM;
    motor->init();

    // Create haptic PID
    haptic_pid = new PIDController(5.0f, 0.0f, 0.004f, 10000.0f, 0.4f);

    // Create haptic interface
    haptic = new HapticInterface(motor, haptic_pid);
    haptic->init();

    return 0;
}

int nanod_motor_calibrate(void) {
    if (!motor) return -1;
    return motor->initFOC();
}

void nanod_motor_set_calibration(int direction, float zero_electric_angle) {
    if (!motor) return;
    motor->sensor_direction = (Direction)direction;
    motor->zero_electric_angle = zero_electric_angle;
    motor->motor_status = FOCMotorStatus::motor_ready;
}

void nanod_motor_get_calibration(int* direction, float* zero_electric_angle) {
    if (!motor) return;
    *direction = (int)motor->sensor_direction;
    *zero_electric_angle = motor->zero_electric_angle;
}

void nanod_motor_set_encoder_angle(float angle) {
    if (!sensor) return;
    sensor->setAngle(angle);
}

void nanod_motor_loop(void) {
    if (!motor || !haptic) return;
    haptic->haptic_loop();
}

uint16_t nanod_motor_get_position(void) {
    if (!haptic) return 0;
    return haptic->haptic_state.current_pos;
}

float nanod_motor_get_shaft_angle(void) {
    if (!motor) return 0.0f;
    return motor->shaft_angle;
}

float nanod_motor_get_shaft_velocity(void) {
    if (!motor) return 0.0f;
    return motor->shaft_velocity;
}

void nanod_motor_load_profile(
    uint8_t mode,
    uint16_t start_pos,
    uint16_t end_pos,
    uint16_t detent_count,
    uint8_t vernier,
    uint8_t kx_force,
    float output_ramp,
    float detent_strength
) {
    if (!haptic) return;

    DetentProfile profile;
    profile.mode = (HapticMode)mode;
    profile.start_pos = start_pos;
    profile.end_pos = end_pos;
    profile.detent_count = detent_count;
    profile.vernier = vernier;
    profile.kxForce = (kx_force != 0);
    profile.output_ramp = output_ramp;
    profile.detent_strength = detent_strength;

    haptic->haptic_state.load_profile(profile, start_pos);

    // Offset detent system to current position
    if (motor) {
        motor->sensor_offset = motor->shaft_angle;
        haptic->haptic_state.attract_angle = 0.0f;
        haptic->haptic_state.last_attract_angle = 0.0f;
    }
}

void nanod_motor_recalibrate(void) {
    if (!motor) return;
    motor->disable();
    delay(500);
    motor->sensor_direction = Direction::UNKNOWN;
    motor->zero_electric_angle = NOT_SET;
    motor->enable();
    motor->initFOC();
}

void nanod_motor_offset_detent(void) {
    if (!motor || !haptic) return;
    motor->sensor_offset = motor->shaft_angle;
    haptic->haptic_state.attract_angle = 0.0f;
    haptic->haptic_state.last_attract_angle = 0.0f;
}

} // extern "C"
