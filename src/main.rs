#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt;
use embassy_time::{Duration, Timer};
// Advertisement builder imports removed - now using advertising controller
use nrf_softdevice::{Config as SdConfig, Softdevice};
use {defmt_rtt as _, panic_probe as _};

mod ble;
mod commands;
mod core;

use core::transport::{RxSpiConfig, TxSpiConfig};

use ble::services::Server;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting nRF52820 S140 firmware");

    // Configure nRF peripherals
    let mut nrf_config = Config::default();
    // Configure interrupt priorities to avoid SoftDevice reserved levels (0, 1, 4)
    nrf_config.gpiote_interrupt_priority = interrupt::Priority::P2;
    nrf_config.time_interrupt_priority = interrupt::Priority::P2;

    let peripherals = embassy_nrf::init(nrf_config);

    info!("Embassy initialized, configuring SoftDevice...");

    // Configure SoftDevice with basic settings
    let sd_config = SdConfig {
        clock: Some(nrf_softdevice::raw::nrf_clock_lf_cfg_t {
            source: nrf_softdevice::raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: nrf_softdevice::raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
        }),
        conn_gap: Some(nrf_softdevice::raw::ble_gap_conn_cfg_t {
            conn_count: 1,
            event_length: 24,
        }),
        conn_gatt: Some(nrf_softdevice::raw::ble_gatt_conn_cfg_t { att_mtu: 247 }),
        gatts_attr_tab_size: Some(nrf_softdevice::raw::ble_gatts_cfg_attr_tab_size_t { attr_tab_size: 1408 }),
        gap_role_count: Some(nrf_softdevice::raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 1,
            central_role_count: 0,
            central_sec_count: 0,
            _bitfield_1: Default::default(),
        }),
        ..Default::default()
    };

    let sd = Softdevice::enable(&sd_config);
    info!("SoftDevice enabled successfully!");

    // Initialize Bluetooth GATT server
    let server = Server::new(sd).unwrap_or_else(|_| {
        defmt::panic!("Failed to initialize Server");
    });
    info!("Server initialized");

    // Spawn SoftDevice task (CRITICAL!)
    unwrap!(spawner.spawn(softdevice_task(sd)));

    // Configure SPI peripherals
    let tx_spi_config = TxSpiConfig {
        ss_pin: peripherals.P0_01,
        sck_pin: peripherals.P0_00,
        mosi_pin: peripherals.P0_04,
        miso_pin: peripherals.P0_02, // Dummy MISO for master mode
    };

    let rx_spi_config = RxSpiConfig {
        ss_pin: peripherals.P0_07,
        sck_pin: peripherals.P0_06,
        mosi_pin: peripherals.P0_05,
        miso_pin: peripherals.P0_03, // Dummy MISO for slave mode
    };

    // Initialize and spawn SPI tasks
    unwrap!(
        core::transport::init_and_spawn(
            &spawner,
            tx_spi_config,
            rx_spi_config,
            peripherals.TWISPI0,
            peripherals.TWISPI1,
        )
        .await
    );

    // Initialize other modules
    ble::gatt_state::init();
    core::memory::init();
    ble::connection::init();
    ble::bonding::init();
    ble::gap_state::init().await;

    // Spawn advertising task (replaces the old BLE task)
    unwrap!(spawner.spawn(ble::advertising::advertising_task(sd, server)));

    // Spawn command processor task to handle SPI commands
    unwrap!(spawner.spawn(commands::command_processor_task(sd)));

    // Spawn service manager task for dynamic GATT operations
    unwrap!(spawner.spawn(ble::manager::service_manager_task(sd)));

    // Spawn notification service task for BLE notifications/indications
    unwrap!(spawner.spawn(ble::notifications::notification_service_task()));

    // Event forwarding is now handled directly in the advertising task

    info!("System initialized, entering main loop");

    // Main loop - just logging heartbeat
    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("Heartbeat - system running");
    }
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}
