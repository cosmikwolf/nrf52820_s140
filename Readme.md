


1. Install probe-rs
cargo install probe-rs-cli

2. install flip-link
cargo install flip-link

3. flash softdevice
probe-rs download --binary-format ihex --chip nRF52820_xxAA s140_nrf52_7.3.0_softdevice.hex

4. build and flash app
cargo run
