//! BLE Protocol Implementation
//! 
//! Contains all BLE-related functionality including GAP and GATT services.
//! This module implements the Bluetooth Low Energy protocol stack components
//! for the nRF52820 firmware.

pub mod advertising;
pub mod bonding;
pub mod connection;
pub mod dynamic;
pub mod events;
pub mod gap_state;
pub mod gatt_state;
pub mod manager;
pub mod notifications;
pub mod registry;
pub mod services;