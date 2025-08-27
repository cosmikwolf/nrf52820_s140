#![no_std]
#![no_main]
#![macro_use]
use defmt_rtt as _; // global logger
use embassy_nrf as _; // time driver
use panic_probe as _;

use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::{config::Config, interrupt};
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
            // source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            // rc_ctiv: 16,
            // rc_temp_ctiv: 2,
            // accuracy: raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
            source: raw::NRF_CLOCK_LF_SRC_SYNTH as u8, // Use synthesized from 32MHz crystal
            rc_ctiv: 0,
            rc_temp_ctiv: 0,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
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
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}
