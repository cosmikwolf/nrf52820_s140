#![no_std]
#![no_main]

// Define our own unwrap! macro that takes precedence over defmt's
mod common {
    #![macro_use]
    use defmt_rtt as _; // global logger
    use embassy_nrf as _; // time driver
    use panic_probe as _;
}
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_nrf::{config::Config, interrupt};
use embassy_time::{Duration, Timer};
use nrf_softdevice::ble::advertisement_builder::{Flag, LegacyAdvertisementBuilder, LegacyAdvertisementPayload};
use nrf_softdevice::ble::peripheral::advertise_connectable;
use nrf_softdevice::ble::{gatt_server, peripheral};
use nrf_softdevice::{raw, Config as SdConfig, Softdevice};

#[nrf_softdevice::gatt_server]
struct TestServer {
    test_service: TestService,
}

#[nrf_softdevice::gatt_service(uuid = "9e7312e0-2354-11eb-9f10-fbc30a62cf38")]
struct TestService {
    #[characteristic(uuid = "9e7312e0-2354-11eb-9f10-fbc30a63cf38", read, write)]
    test_char: u8,
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("=== BLE Connection Test Starting ===");

    // Configure nRF peripherals
    let mut nrf_config = Config::default();
    nrf_config.gpiote_interrupt_priority = interrupt::Priority::P2;
    nrf_config.time_interrupt_priority = interrupt::Priority::P2;
    let _peripherals = embassy_nrf::init(nrf_config);
    info!("Embassy initialized");

    // Configure SoftDevice with proper connection parameters
    let sd_config = SdConfig {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_SYNTH as u8, // Use synthesized from 32MHz crystal
            rc_ctiv: 0,
            rc_temp_ctiv: 0,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_50_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 2, // Balanced - more than minimal but less than full example
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t {
            att_mtu: 128, // Match working example
        }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: raw::BLE_GATTS_ATTR_TAB_SIZE_DEFAULT, // Use default like working example
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 2,
            central_role_count: 1,
            central_sec_count: 0,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        // Configure vendor-specific UUID support (matches original firmware)
        common_vs_uuid: Some(raw::ble_common_cfg_vs_uuid_t { vs_uuid_count: 10 }),
        gap_device_name: None,
        gap_ppcp_incl: None,
        gap_car_incl: None,
        gatts_service_changed: None,
        conn_gattc: None,
        conn_gatts: None,
    };

    let sd = Softdevice::enable(&sd_config);
    info!("SoftDevice enabled with SYNTH clock");

    // Initialize test GATT server
    info!("About to create GATT server...");
    let server = TestServer::new(sd).unwrap();
    info!("Test GATT server initialized");

    // Spawn SoftDevice task
    info!("About to spawn softdevice_task...");
    unwrap!(spawner.spawn(softdevice_task(sd)));
    info!("softdevice_task spawned");

    // Spawn BLE connection test task
    info!("About to spawn ble_connection_test_task...");
    unwrap!(spawner.spawn(ble_connection_test_task(sd, server)));
    info!("ble_connection_test_task spawned");

    info!("All tasks started - entering main loop");

    // Main loop - just heartbeat
    loop {
        Timer::after(Duration::from_secs(5)).await;
        info!("=== Test running - heartbeat ===");
    }
}

#[embassy_executor::task]
async fn ble_connection_test_task(sd: &'static Softdevice, server: TestServer) {
    info!("Starting BLE connection test...");

    // Create advertisement data
    static ADV_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
        .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
        .full_name("BLE-Test")
        .build();

    static SCAN_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new().build();

    let mut connection_count = 0u32;

    loop {
        info!(">>> Starting BLE advertising (attempt #{})...", connection_count + 1);

        // Create advertising config with connection parameters
        let config = peripheral::Config {
            interval: 150,
            ..Default::default()
        };
        // 100ms advertising interval

        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &ADV_DATA,
            scan_data: &SCAN_DATA,
        };

        // Start advertising and wait for connection - match example pattern exactly
        let conn = unwrap!(advertise_connectable(sd, adv, &config).await);

        connection_count += 1;
        info!("✓ CONNECTION #{} ESTABLISHED!", connection_count);
        info!("  Peer address: {:?}", defmt::Debug2Format(&conn.peer_address()));
        info!("  Connection handle: {:?}", conn.handle());

        info!(">>> Connection active - waiting for GATT events or disconnection...");

        // Run GATT server - this blocks until disconnection
        let disconnect_result = gatt_server::run(&conn, &server, |event| {
            match event {
                TestServerEvent::TestService(TestServiceEvent::TestCharWrite(value)) => {
                    info!("📝 GATT Write received: test_char = {}", value);
                } // Note: CCCD events are not generated for simple read/write characteristics
                  // This would only be needed if we had notify/indicate enabled
            }
        })
        .await;

        // Connection ended - gatt_server::run returns DisconnectedError when connection ends
        info!("✗ Connection ended: {:?}", defmt::Debug2Format(&disconnect_result));

        info!(">>> CONNECTION #{} DISCONNECTED", connection_count);
        info!("    Total connections so far: {}", connection_count);
    }
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}
