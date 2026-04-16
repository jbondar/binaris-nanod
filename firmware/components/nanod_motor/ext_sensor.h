// External sensor — angle provided from Rust via C API.
// This replaces the Arduino SPI-based MT6701 driver.
#pragma once

#include "simplefoc/Sensor.h"

class ExternalSensor : public Sensor {
public:
    ExternalSensor() : _angle(0), _velocity(0), _full_rotation_offset(0), _prev_angle(0) {}

    void init() override {}

    // Called by Rust to feed encoder angle
    void setAngle(float angle) {
        _prev_angle = _angle;
        _angle = angle;
    }

    // SimpleFOC Sensor interface
    float getSensorAngle() override {
        return _angle;
    }

private:
    float _angle;
    float _velocity;
    float _full_rotation_offset;
    float _prev_angle;
};
