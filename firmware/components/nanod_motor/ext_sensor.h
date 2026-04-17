// Sensor that reads MT6701 via SPI directly from C++.
// Used by SimpleFOC for both normal operation AND calibration.
#pragma once

#include "simplefoc/Sensor.h"
#include "driver/spi_master.h"

#define MT6701_RESOLUTION 16384.0f

class Mt6701Sensor : public Sensor {
public:
    Mt6701Sensor() : _spi_device(nullptr) {}

    // Initialize SPI for direct encoder reads
    bool initSPI(int cs_pin, int sclk_pin, int miso_pin) {
        spi_bus_config_t bus_config = {};
        bus_config.mosi_io_num = -1;
        bus_config.miso_io_num = miso_pin;
        bus_config.sclk_io_num = sclk_pin;
        bus_config.quadwp_io_num = -1;
        bus_config.quadhd_io_num = -1;
        bus_config.max_transfer_sz = 32;
        bus_config.flags = SPICOMMON_BUSFLAG_MASTER;

        // Use SPI2 for encoder (SPI3 is used by display)
        esp_err_t err = spi_bus_initialize(SPI2_HOST, &bus_config, 0);
        if (err != ESP_OK && err != ESP_ERR_INVALID_STATE) return false;

        spi_device_interface_config_t dev_config = {};
        dev_config.clock_speed_hz = 10000000; // 10MHz
        dev_config.mode = 3; // CPOL=1 CPHA=1
        dev_config.spics_io_num = cs_pin;
        dev_config.queue_size = 1;
        dev_config.flags = SPI_DEVICE_HALFDUPLEX;

        err = spi_bus_add_device(SPI2_HOST, &dev_config, &_spi_device);
        if (err != ESP_OK) return false;

        // Pre-acquire bus for fast polling
        spi_device_acquire_bus(_spi_device, portMAX_DELAY);
        return true;
    }

    void init() override {}

    float getSensorAngle() override {
        if (!_spi_device) return 0.0f;

        uint8_t rx_buf[4] = {0};
        spi_transaction_t trans = {};
        trans.rxlength = 24;
        trans.length = 0;
        trans.rx_buffer = rx_buf;

        spi_device_polling_transmit(_spi_device, &trans);

        uint16_t raw = ((uint16_t)rx_buf[0] << 6) | ((uint16_t)rx_buf[1] >> 2);
        raw &= 0x3FFF;

        return (float)raw / MT6701_RESOLUTION * 6.28318530718f;
    }

private:
    spi_device_handle_t _spi_device;
};
