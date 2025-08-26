#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{interrupt, config::Config};
use embassy_time::{Duration, Timer};
use nrf_softdevice::{Softdevice, Config as SdConfig};
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
    let mgmt_server = unwrap!(ManagementServer::new(sd));
    info!("Management service initialized");

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
        let mut adv_data = heapless::Vec::<u8, 31>::new();
        unwrap!(adv_data.extend_from_slice(&[
            0x02, 0x01, 0x06, // Flags: General discoverable, BR/EDR not supported
        ]));
        unwrap!(adv_data.extend_from_slice(&[
            0x0C, 0x09, // Complete local name (12 bytes + type)
        ]));
        unwrap!(adv_data.extend_from_slice(b"MDBT50-Demo"));

        let mut scan_data = heapless::Vec::<u8, 31>::new();
        
        let config = nrf_softdevice::ble::peripheral::Config::default();
        let advertisement = nrf_softdevice::ble::peripheral::ConnectableAdvertisement::ScannableUndirected { 
            adv_data: &adv_data, 
            scan_data: &scan_data 
        };
        
        let adv = nrf_softdevice::ble::peripheral::advertise_connectable(
            sd, 
            advertisement,
            &config
        ).await;
        
        match adv {
            Ok(conn) => {
                info!("BLE connected!");
                
                // Handle the connection with management service
                let result = connection_task(&conn, &mgmt_server).await;
                match result {
                    Ok(_) => info!("Connection ended normally"),
                    Err(_) => info!("Connection disconnected"),
                }
            }
            Err(_) => {
                error!("BLE advertising error");
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn connection_task(
    conn: &nrf_softdevice::ble::Connection,
    mgmt_server: &ManagementServer,
) -> Result<(), nrf_softdevice::ble::DisconnectedError> {
    info!("Connection established, starting management service...");
    
    // Run the management service for this connection
    mgmt_server.run(conn).await
}
