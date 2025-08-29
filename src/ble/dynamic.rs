//! Dynamic GATT Server Implementation
//!
//! This module provides a GATT server that can handle dynamically created
//! services and characteristics, forwarding events to the host via the
//! BLE modem protocol.

use defmt::{debug, info, warn, Format};
use nrf_softdevice::ble::gatt_server::{self, WriteOp};
use nrf_softdevice::ble::{Connection, Uuid};
use nrf_softdevice::Softdevice;

use crate::ble::events::{self, BleModemEvent};
use crate::ble::registry::{with_registry, BleUuid, GattRegistry};

/// Dynamic GATT server events
#[derive(Debug, Format)]
pub enum DynamicGattEvent {
    /// Characteristic was written by client
    CharacteristicWrite {
        conn_handle: u16,
        char_handle: u16,
        data_len: u8, // Store length instead of Vec to avoid Format issues
    },
    /// CCCD was written (notifications/indications enabled/disabled)
    CccdWrite {
        conn_handle: u16,
        char_handle: u16,
        notifications: bool,
        indications: bool,
    },
    /// MTU was exchanged
    MtuExchange { conn_handle: u16, mtu: u16 },
}

/// Dynamic GATT server implementation
///
/// This server supports runtime service and characteristic creation
/// and forwards all GATT events to the host via the BLE modem protocol.
pub struct DynamicGattServer {
    // Future: could add server-level state here if needed
}

impl DynamicGattServer {
    /// Create a new dynamic GATT server
    pub fn new(_sd: &mut Softdevice) -> Result<Self, gatt_server::RegisterError> {
        info!("Creating dynamic GATT server");

        Ok(Self {
            // No static services for now - all created dynamically
        })
    }

    /// Get server statistics
    pub fn stats(&self) -> (u8, u8, u8) {
        with_registry(|registry| registry.stats())
    }

    /// Handle a characteristic write event
    async fn handle_write_event(
        &self,
        conn: &Connection,
        handle: u16,
        _op: WriteOp,
        _offset: usize,
        data: &[u8],
    ) -> Option<DynamicGattEvent> {
        let conn_handle = conn.handle().unwrap_or(0);

        // Check if this is a CCCD write
        if let Some(char_info) =
            with_registry(|registry| registry.find_characteristic_by_cccd_handle(handle).map(|c| *c))
        {
            debug!("CCCD write on handle {}: {} bytes", handle, data.len());

            if !data.is_empty() {
                let notifications = (data[0] & 0x01) != 0;
                let indications = (data[0] & 0x02) != 0;

                debug!("CCCD: notifications={}, indications={}", notifications, indications);

                // Forward CCCD write event to host
                let cccd_event =
                    events::create_cccd_write_event(conn_handle, char_info.value_handle, notifications, indications);

                if let Err(_) = events::forward_event_to_host(cccd_event).await {
                    warn!("Failed to forward CCCD write event to host");
                }

                return Some(DynamicGattEvent::CccdWrite {
                    conn_handle,
                    char_handle: char_info.value_handle,
                    notifications,
                    indications,
                });
            }
        }

        // Check if this is a characteristic value write
        if let Some(char_info) =
            with_registry(|registry| registry.find_characteristic_by_value_handle(handle).map(|c| *c))
        {
            debug!("Characteristic write on handle {}: {} bytes", handle, data.len());

            // Forward write event to host
            let write_event = events::create_gatts_write_event(conn_handle, handle, data);

            match write_event {
                Ok(event) => {
                    if let Err(_) = events::forward_event_to_host(event).await {
                        warn!("Failed to forward write event to host");
                    }
                }
                Err(_) => {
                    warn!("Failed to create write event (data too large?)");
                }
            }

            return Some(DynamicGattEvent::CharacteristicWrite {
                conn_handle,
                char_handle: handle,
                data_len: data.len().min(255) as u8, // Store length only
            });
        }

        debug!("Write to unknown handle {}", handle);
        None
    }
}

impl gatt_server::Server for DynamicGattServer {
    type Event = DynamicGattEvent;

    fn on_write(&self, conn: &Connection, handle: u16, op: WriteOp, offset: usize, data: &[u8]) -> Option<Self::Event> {
        // Note: We can't use async in this trait method, so we'll spawn
        // the event forwarding in a separate context. For now, just
        // create the event and let the caller handle forwarding.

        let conn_handle = conn.handle().unwrap_or(0);

        // Check if this is a CCCD write
        if let Some(char_info) =
            with_registry(|registry| registry.find_characteristic_by_cccd_handle(handle).map(|c| *c))
        {
            debug!("CCCD write on handle {}: {} bytes", handle, data.len());

            if !data.is_empty() {
                let notifications = (data[0] & 0x01) != 0;
                let indications = (data[0] & 0x02) != 0;

                debug!("CCCD: notifications={}, indications={}", notifications, indications);

                return Some(DynamicGattEvent::CccdWrite {
                    conn_handle,
                    char_handle: char_info.value_handle,
                    notifications,
                    indications,
                });
            }
        }

        // Check if this is a characteristic value write
        if let Some(_char_info) =
            with_registry(|registry| registry.find_characteristic_by_value_handle(handle).map(|c| *c))
        {
            debug!("Characteristic write on handle {}: {} bytes", handle, data.len());

            return Some(DynamicGattEvent::CharacteristicWrite {
                conn_handle,
                char_handle: handle,
                data_len: data.len().min(255) as u8, // Store length only
            });
        }

        debug!(
            "Write to unknown handle {} (op: {:?}, offset: {}, len: {})",
            handle,
            op,
            offset,
            data.len()
        );
        None
    }
}

/// Utility functions for working with dynamic services
pub mod utils {
    use nrf_softdevice::ble::gatt_server::builder::ServiceBuilder;
    use nrf_softdevice::ble::gatt_server::characteristic::{Attribute, Metadata, Properties};
    use nrf_softdevice::ble::gatt_server::CharacteristicHandles;

    use super::*;
    use crate::ble::registry::{char_properties, ServiceType};

    /// Create a service using ServiceBuilder
    pub fn create_service(
        sd: &mut Softdevice,
        uuid: Uuid,
        service_type: ServiceType,
    ) -> Result<u16, gatt_server::RegisterError> {
        let mut sb = ServiceBuilder::new(sd, uuid)?;

        // For secondary services, we would need to set that in the builder
        // but the current nrf-softdevice API only supports primary services
        // via ServiceBuilder::new()

        let service_handle = sb.build();
        let handle_value = service_handle.handle();

        info!("Created service with handle {}", handle_value);
        Ok(handle_value)
    }

    /// Add a characteristic to an existing service builder
    pub fn add_characteristic_to_builder(
        sb: &mut ServiceBuilder,
        uuid: Uuid,
        properties: u8,
        initial_value: &[u8],
        _permissions: u8, // TODO: Use permissions when supported
    ) -> Result<CharacteristicHandles, gatt_server::RegisterError> {
        // Convert properties byte to Properties struct
        let mut props = Properties::new();

        if properties & char_properties::READ != 0 {
            props = props.read();
        }
        if properties & char_properties::WRITE != 0 {
            props = props.write();
        }
        if properties & char_properties::WRITE_WITHOUT_RESPONSE != 0 {
            props = props.write_without_response();
        }
        if properties & char_properties::NOTIFY != 0 {
            props = props.notify();
        }
        if properties & char_properties::INDICATE != 0 {
            props = props.indicate();
        }
        if properties & char_properties::BROADCAST != 0 {
            props = props.broadcast();
        }
        if properties & char_properties::AUTH_SIGNED_WRITES != 0 {
            props = props.signed_write();
        }

        let attr = Attribute::new(initial_value);
        let metadata = Metadata::new(props);

        let char_builder = sb.add_characteristic(uuid, attr, metadata)?;
        let handles = char_builder.build();

        debug!(
            "Added characteristic - value_handle: {}, cccd_handle: {}",
            handles.value_handle, handles.cccd_handle
        );

        Ok(handles)
    }

    /// Send a notification to a connected client
    pub fn send_notification(conn: &Connection, handle: u16, data: &[u8]) -> Result<(), gatt_server::NotifyValueError> {
        gatt_server::notify_value(conn, handle, data)
    }

    /// Send an indication to a connected client
    pub fn send_indication(conn: &Connection, handle: u16, data: &[u8]) -> Result<(), gatt_server::IndicateValueError> {
        gatt_server::indicate_value(conn, handle, data)
    }

    /// Get characteristic value
    pub fn get_characteristic_value(
        sd: &Softdevice,
        handle: u16,
        buffer: &mut [u8],
    ) -> Result<usize, gatt_server::GetValueError> {
        gatt_server::get_value(sd, handle, buffer)
    }

    /// Set characteristic value
    pub fn set_characteristic_value(
        sd: &Softdevice,
        handle: u16,
        data: &[u8],
    ) -> Result<(), gatt_server::SetValueError> {
        gatt_server::set_value(sd, handle, data)
    }
}
