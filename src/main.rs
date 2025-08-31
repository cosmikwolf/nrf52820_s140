#![no_std]
#![no_main]

use defmt::{error, info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::yield_now;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt;
// Advertisement builder imports removed - now using advertising controller
use nrf_softdevice::{Config as SdConfig, Softdevice};
use {defmt_rtt as _, panic_probe as _};

// panic-probe provides the panic handler with RTT output and debugger support

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

    // Configure SoftDevice with working test_connection.rs settings
    let sd_config = SdConfig {
        clock: Some(nrf_softdevice::raw::nrf_clock_lf_cfg_t {
            source: nrf_softdevice::raw::NRF_CLOCK_LF_SRC_SYNTH as u8, // Use synthesized from 32MHz crystal
            rc_ctiv: 0,
            rc_temp_ctiv: 0,
            accuracy: nrf_softdevice::raw::NRF_CLOCK_LF_ACCURACY_50_PPM as u8,
        }),
        conn_gap: Some(nrf_softdevice::raw::ble_gap_conn_cfg_t {
            conn_count: 2, // Balanced - more than minimal but less than full example
            event_length: 24,
        }),
        conn_gatt: Some(nrf_softdevice::raw::ble_gatt_conn_cfg_t {
            att_mtu: 128, // Match working example
        }),
        gatts_attr_tab_size: Some(nrf_softdevice::raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: nrf_softdevice::raw::BLE_GATTS_ATTR_TAB_SIZE_DEFAULT, // Use default like working example
        }),
        gap_role_count: Some(nrf_softdevice::raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 2,
            central_role_count: 1,
            central_sec_count: 0,
            _bitfield_1: nrf_softdevice::raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        // Configure vendor-specific UUID support (matches original firmware)
        common_vs_uuid: Some(nrf_softdevice::raw::ble_common_cfg_vs_uuid_t { vs_uuid_count: 10 }),
        gap_device_name: None,
        gap_ppcp_incl: None,
        gap_car_incl: None,
        gatts_service_changed: None,
        conn_gattc: None,
        conn_gatts: None,
    };

    let sd = Softdevice::enable(&sd_config);
    info!("SoftDevice enabled successfully!");

    // Initialize Bluetooth GATT server
    let server = Server::new(sd).unwrap_or_else(|_| {
        defmt::panic!("Failed to initialize Server");
    });
    info!("Server initialized");

    // Spawn SoftDevice task (CRITICAL for timer functionality!)
    unwrap!(spawner.spawn(softdevice_task(sd)));

    // Configure SPI peripherals
    let tx_spi_config = TxSpiConfig {
        cs_pin: peripherals.P0_01,
        sck_pin: peripherals.P0_00,
        mosi_pin: peripherals.P0_04, // Master out - device transmits to host
    };

    let rx_spi_config = RxSpiConfig {
        cs_pin: peripherals.P0_07,
        sck_pin: peripherals.P0_06,
        miso_pin: peripherals.P0_05, // Slave in - host transmits to device
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

    // // Initialize other modules
    ble::gatt_state::init();
    core::memory::init();
    ble::connection::init();
    ble::bonding::init();
    ble::gap_state::init().await;
    //
    // // Spawn advertising task (replaces the old BLE task)
    // info!("Spawning advertising task...");
    unwrap!(spawner.spawn(ble::advertising::advertising_task(sd, server)));
    //
    // // Spawn command processor task to handle SPI commands
    // info!("Spawning command processor task...");
    unwrap!(spawner.spawn(commands::command_processor_task(sd)));
    //
    // // Spawn service manager task for dynamic GATT operations
    // info!("Spawning service manager task...");
    unwrap!(spawner.spawn(ble::manager::service_manager_task(sd)));
    //
    // // Spawn notification service task for BLE notifications/indications
    // info!("Spawning notification service task...");
    unwrap!(spawner.spawn(ble::notifications::notification_service_task()));
    //
    // Event forwarding is now handled directly in the advertising task

    info!("Main thread starting heartbeat loop...");
    unwrap!(spawner.spawn(heartbeat_task()));

    // Simulate advertising start command for testing
    use ble::advertising::{send_command, AdvCommand};
    let start_cmd = AdvCommand::Start {
        handle: 1,  // Use valid handle (not 0)
        conn_cfg_tag: 0,
    };
    if send_command(start_cmd).is_ok() {
        info!("Simulated advertising start command sent with handle 1");
    } else {
        info!("Failed to send advertising start command");
    }

    info!("System ready - all tasks spawned and running");
    // info!("BLE modem firmware operational - waiting for SPI host commands");
    //
    // Simple heartbeat using yield_now to avoid timer issues
    // let mut counter = 0;
    loop {
        //     // Yield to allow other tasks to run
        //     // for _ in 0..100000 {
        //     //     yield_now().await;
        //     // }
        //
        //     counter += 1;
        //     // if counter % 100 == 0 {
        //     info!("Heartbeat: System operational ({} cycles)", counter);
        //     // }
        // Simple delay using yield_now loop
        for _ in 0..100000 {
            yield_now().await;
        }
    }
}

#[embassy_executor::task]
async fn heartbeat_task() {
    let mut counter = 0;
    loop {
        // Simple delay using yield_now loop
        for _ in 0..100000 {
            yield_now().await;
        }
        info!("Heartbeat: System operational ({} cycles)", counter);
        counter += 1;
    }
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}
