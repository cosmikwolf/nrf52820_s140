#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{config::Config, interrupt};
use embassy_time::{Duration, Timer};
use nrf_softdevice::ble::advertisement_builder::{
    Flag, LegacyAdvertisementBuilder, LegacyAdvertisementPayload,
};
use nrf_softdevice::{Config as SdConfig, Softdevice};
use panic_probe as _;

mod services;
// Temporarily disable modules while fixing compilation
// mod protocol;
// mod buffer_pool;
// mod spi_comm;
// mod state;
// mod commands;

use services::Server;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting nRF52820 S140 firmware");

    // Configure nRF peripherals
    let mut nrf_config = Config::default();
    // Configure interrupt priorities to avoid SoftDevice reserved levels (0, 1, 4)
    nrf_config.gpiote_interrupt_priority = interrupt::Priority::P2;
    nrf_config.time_interrupt_priority = interrupt::Priority::P2;

    let _peripherals = embassy_nrf::init(nrf_config);

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
        gatts_attr_tab_size: Some(nrf_softdevice::raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 1408,
        }),
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
    let bt_server = Server::new(sd).unwrap_or_else(|_| {
        defmt::panic!("Failed to initialize BT server");
    });
    info!("BT server initialized");

    // Spawn SoftDevice task (CRITICAL!)
    unwrap!(spawner.spawn(softdevice_task(sd)));

    // Spawn BLE task
    unwrap!(spawner.spawn(ble_task(sd, bt_server)));

    info!("System initialized, entering main loop");

    // Main loop - just logging heartbeat
    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("Heartbeat - system running");
    }
}

#[embassy_executor::task]
async fn ble_task(sd: &'static Softdevice, bt_server: Server) {
    info!("Starting BLE task...");

    info!("Starting BLE advertising...");

    // Create advertisement data using the builder pattern
    static ADV_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
        .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
        .full_name("MDBT50-Demo")
        .build();

    static SCAN_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new().build();

    loop {
        // Create fresh advertising config for each iteration
        // This matches the official nrf-softdevice example pattern
        let config = nrf_softdevice::ble::peripheral::Config::default();

        let adv = nrf_softdevice::ble::peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &ADV_DATA,
            scan_data: &SCAN_DATA,
        };

        let conn =
            match nrf_softdevice::ble::peripheral::advertise_connectable(sd, adv, &config).await {
                Ok(conn) => conn,
                Err(e) => {
                    error!("BLE advertising failed: {:?}", defmt::Debug2Format(&e));
                    Timer::after(Duration::from_secs(1)).await;
                    continue;
                }
            };

        info!("advertising done!");

        // Run the GATT server on the connection. This returns when the connection gets disconnected.
        use nrf_softdevice::ble::gatt_server;
        let e = gatt_server::run(&conn, &bt_server, |_| {}).await;

        info!(
            "gatt_server run exited with error: {:?}",
            defmt::Debug2Format(&e)
        );
    }
}

// connection_task removed - now using gatt_server::run directly

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}
