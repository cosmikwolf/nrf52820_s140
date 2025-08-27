#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{config::Config, interrupt};
use embassy_time::{Duration, Timer};
use nrf_softdevice::{Config as SdConfig, Softdevice};
use panic_probe as _;

mod mgmt_service;
use mgmt_service::ManagementServer;

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

    // Initialize management service
    let mgmt_server = ManagementServer::new(sd).unwrap_or_else(|_| {
        defmt::panic!("Failed to initialize management service");
    });
    info!("Management service initialized");

    // Spawn SoftDevice task (CRITICAL!)
    unwrap!(spawner.spawn(softdevice_task(sd)));

    // Spawn BLE task
    unwrap!(spawner.spawn(ble_task(sd, mgmt_server)));

    info!("System initialized, entering main loop");

    // Main loop - just logging heartbeat
    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("Heartbeat - system running");
    }
}

#[embassy_executor::task]
async fn ble_task(sd: &'static Softdevice, mgmt_server: ManagementServer) {
    info!("Starting BLE task...");

    info!("Starting BLE advertising...");

    loop {
        // Create advertising data with correct format
        let adv_data = [
            0x02, 0x01, 0x06, // Flags: General discoverable, BR/EDR not supported
            0x0C,
            0x09, // Complete local name (11 bytes + type byte = 12 total, length = 0x0C)
            b'M', b'D', b'B', b'T', b'5', b'0', b'-', b'D', b'e', b'm', b'o',
        ];

        let scan_data: [u8; 0] = [];

        let config = nrf_softdevice::ble::peripheral::Config::default();
        let advertisement =
            nrf_softdevice::ble::peripheral::ConnectableAdvertisement::ScannableUndirected {
                adv_data: &adv_data,
                scan_data: &scan_data,
            };

        let adv =
            nrf_softdevice::ble::peripheral::advertise_connectable(sd, advertisement, &config)
                .await;

        match adv {
            Ok(conn) => {
                info!("BLE connected!");

                // Use the same pattern as the working example
                // gatt_server::run automatically returns when disconnected
                use nrf_softdevice::ble::gatt_server;
                let _ = gatt_server::run(&conn, &mgmt_server, |_| {}).await;

                info!("Connection disconnected, restarting advertising...");
            }
            Err(_) => {
                error!("BLE advertising error");
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }
}

// connection_task removed - now using gatt_server::run directly

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}
