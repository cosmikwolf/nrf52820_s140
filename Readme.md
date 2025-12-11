## nRF52820 Ble Peripheral Example

### * work in progress *
this firmware allows end users to create new Bluetooth services with GATT - some parts of this firmware were written with Claude Code

  1. Install probe-rs

    ```cargo install probe-rs-cli```

  2. install flip-link

    ```cargo install flip-link```

  3. download and s140_nrf52_7.3.0_softdevice.hex from Nordic's website:

    ```https://www.nordicsemi.com/Products/Development-software/S130/Download```

  3. flash softdevice

    ```probe-rs download --binary-format ihex --chip nRF52820_xxAA s140_nrf52_7.3.0_softdevice.hex```

  4. build and flash app

    ```cargo run```

### GATT Operations Implemented over SPI
1. Register UUID bases (REGISTER_UUID_GROUP - 0x0010) - For custom 128-bit UUIDs
2. Create services (GATTS_SERVICE_ADD - 0x0080) - Primary or secondary services
3. Add characteristics (GATTS_CHARACTERISTIC_ADD - 0x0081) - With properties like READ, WRITE, NOTIFY
4. Send notifications (GATTS_HVX - 0x0083) - Push data to connected clients

### Tests

Tests are implemented using `cargo test` and `cargo-embed` to run on the nRF52820 device.
