#![no_std]

//! nRF52820 S140 BLE Modem Firmware Library
//!
//! This library provides the core functionality for the BLE modem firmware,
//! organized into clear architectural layers:
//!
//! - `core`: System infrastructure (memory, protocol, transport)
//! - `ble`: BLE protocol implementation (GAP, GATT services)
//! - `commands`: Application command processing

pub mod ble;
pub mod commands;
pub mod core;
