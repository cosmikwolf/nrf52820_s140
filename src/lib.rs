#![no_std]

//! nRF52820 S140 BLE Modem Firmware Library
//! 
//! This library provides the core functionality for the BLE modem firmware,
//! exposing modules for testing and reuse.

pub mod advertising;
pub mod buffer_pool;
pub mod commands;
pub mod dynamic_gatt;
pub mod events;
pub mod gap_state;
pub mod gatt_registry;
pub mod protocol;
pub mod services;
pub mod spi_comm;
pub mod state;