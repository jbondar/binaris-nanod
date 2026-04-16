// Minimal Arduino compatibility shim for SimpleFOC on ESP-IDF.
// Only provides the functions SimpleFOC actually uses.
#pragma once

#include <cstdint>
#include <cmath>
#include <cstring>
#include "esp_timer.h"
#include "driver/gpio.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

// Arduino pin modes
#define INPUT 0x01
#define OUTPUT 0x03
#define INPUT_PULLUP 0x05

// Arduino HIGH/LOW
#define HIGH 1
#define LOW  0

// Arduino-style functions
inline void pinMode(int pin, int mode) {
    if (pin < 0) return;
    gpio_config_t cfg = {};
    cfg.pin_bit_mask = 1ULL << pin;
    cfg.intr_type = GPIO_INTR_DISABLE;
    cfg.pull_up_en = GPIO_PULLUP_DISABLE;
    cfg.pull_down_en = GPIO_PULLDOWN_DISABLE;
    if (mode == OUTPUT) {
        cfg.mode = GPIO_MODE_OUTPUT;
    } else if (mode == INPUT_PULLUP) {
        cfg.mode = GPIO_MODE_INPUT;
        cfg.pull_up_en = GPIO_PULLUP_ENABLE;
    } else {
        cfg.mode = GPIO_MODE_INPUT;
    }
    gpio_config(&cfg);
}

inline void digitalWrite(int pin, int value) {
    if (pin < 0) return;
    gpio_set_level((gpio_num_t)pin, value);
}

inline int digitalRead(int pin) {
    return gpio_get_level((gpio_num_t)pin);
}

inline unsigned long millis() {
    return (unsigned long)(esp_timer_get_time() / 1000);
}

inline unsigned long micros() {
    return (unsigned long)esp_timer_get_time();
}

inline void delay(unsigned long ms) {
    vTaskDelay(ms / portTICK_PERIOD_MS);
}

inline void delayMicroseconds(unsigned long us) {
    int64_t end = esp_timer_get_time() + us;
    while (esp_timer_get_time() < end) {}
}

// Arduino LEDC PWM shims (ESP-IDF native LEDC driver)
#include "driver/ledc.h"

inline double ledcSetup(uint8_t channel, double freq, uint8_t resolution) {
    ledc_timer_config_t timer_conf = {};
    timer_conf.speed_mode = LEDC_LOW_SPEED_MODE;
    timer_conf.duty_resolution = (ledc_timer_bit_t)resolution;
    timer_conf.timer_num = (ledc_timer_t)(channel / 2); // 2 channels per timer
    timer_conf.freq_hz = (uint32_t)freq;
    timer_conf.clk_cfg = LEDC_AUTO_CLK;
    ledc_timer_config(&timer_conf);
    return freq;
}

inline void ledcAttachPin(int pin, uint8_t channel) {
    ledc_channel_config_t ch_conf = {};
    ch_conf.gpio_num = pin;
    ch_conf.speed_mode = LEDC_LOW_SPEED_MODE;
    ch_conf.channel = (ledc_channel_t)channel;
    ch_conf.timer_sel = (ledc_timer_t)(channel / 2);
    ch_conf.duty = 0;
    ch_conf.hpoint = 0;
    ledc_channel_config(&ch_conf);
}

inline void ledcWrite(uint8_t channel, uint32_t duty) {
    ledc_set_duty(LEDC_LOW_SPEED_MODE, (ledc_channel_t)channel, duty);
    ledc_update_duty(LEDC_LOW_SPEED_MODE, (ledc_channel_t)channel);
}

// Minimal Print class stub (SimpleFOC debug uses it but we disable debug)
#define SIMPLEFOC_DISABLE_DEBUG

// Arduino constrain/min/max
#define constrain(x, lo, hi) ((x)<(lo)?(lo):((x)>(hi)?(hi):(x)))
// String class stub (haptic_api.h uses it)
// Must include STL headers BEFORE defining min/max macros
#include <string>
#include <algorithm>
using String = std::string;

// Arduino min/max — defined AFTER STL includes to avoid conflicts
#define min(a,b) ((a)<(b)?(a):(b))
#define max(a,b) ((a)>(b)?(a):(b))

// NOT_SET sentinel
#ifndef NOT_SET
#define NOT_SET (-12345.0f)
#endif

// _isset macro from SimpleFOC
#ifndef _isset
#define _isset(a) ( (a) != (float)NOT_SET )
#endif

// Arduino math helpers
#ifndef radians
#define radians(deg) ((deg) * M_PI / 180.0f)
#endif
#ifndef degrees
#define degrees(rad) ((rad) * 180.0f / M_PI)
#endif
#ifndef M_PI
#define M_PI 3.14159265358979323846f
#endif

// Print class stub for SimpleFOC monitoring (we don't use it)
class Print {
public:
    virtual size_t write(uint8_t) { return 0; }
    virtual size_t write(const uint8_t*, size_t) { return 0; }
    void print(const char*) {}
    void print(float, unsigned int = 2) {}
    void print(int) {}
    void println(const char*) {}
    void println(float, unsigned int = 2) {}
    void println(int) {}
    void println() {}
};

// Serial stub
class SerialStub : public Print {
public:
    operator bool() const { return false; }
};
static SerialStub Serial;
