idf_component_register(
    SRCS
        "nanod_motor.cpp"
        "haptic.cpp"
        "simplefoc/BLDCMotor.cpp"
        "simplefoc/FOCMotor.cpp"
        "simplefoc/Sensor.cpp"
        "simplefoc/CurrentSense.cpp"
        "simplefoc/BLDCDriver3PWM.cpp"
        "simplefoc/foc_utils.cpp"
        "simplefoc/time_utils.cpp"
        "simplefoc/pid.cpp"
        "simplefoc/lowpass_filter.cpp"
        "simplefoc/esp32_mcu.cpp"
        "simplefoc/esp32_ledc_mcu.cpp"
    INCLUDE_DIRS
        "include"
        "."
        "simplefoc"
    REQUIRES
        driver
        esp_timer
)

# Compile options for C++ component
target_compile_options(${COMPONENT_LIB} PRIVATE
    -DSIMPLEFOC_DISABLE_DEBUG
    -DESP32
    -DARDUINO_ARCH_ESP32
    -DSIMPLEFOC_ESP32_USELEDC
    -DESP_H
    -Wno-unused-variable
    -Wno-sign-compare
    -Wno-unused-function
    -Wno-macro-redefined
)
