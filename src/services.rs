//! GATT Server Services Module
//!
//! This module provides the main GATT server implementation,
//! supporting dynamic service creation via the BLE modem protocol.

use defmt::{info, error};
use nrf_softdevice::ble::gatt_server::{self, WriteOp};
use nrf_softdevice::ble::Connection;
use nrf_softdevice::Softdevice;

use crate::{
    events,
};
use crate::dynamic_gatt::{DynamicGattServer, DynamicGattEvent};

/// Main GATT Server implementation
/// 
/// This server combines any static services (currently none) with
/// dynamic services created at runtime via protocol commands.
pub struct Server {
    dynamic_server: DynamicGattServer,
}

impl Server {
    /// Create a new GATT server
    pub fn new(sd: &mut Softdevice) -> Result<Self, gatt_server::RegisterError> {
        info!("Initializing GATT server with dynamic service support");
        
        let dynamic_server = DynamicGattServer::new(sd)?;
        
        Ok(Self {
            dynamic_server,
        })
    }
    
    /// Get server statistics (services, characteristics, UUID bases)
    pub fn stats(&self) -> (u8, u8, u8) {
        self.dynamic_server.stats()
    }
    
    /// Handle a GATT server event and forward to host if needed
    async fn handle_gatt_event(&self, event: DynamicGattEvent) {
        match event {
            DynamicGattEvent::CharacteristicWrite { conn_handle, char_handle, data_len } => {
                // Note: We only store the data length in the event
                // The actual data forwarding is handled in the DynamicGattServer::on_write method
                info!("Characteristic write: conn={}, char={}, len={}", conn_handle, char_handle, data_len);
            },
            DynamicGattEvent::CccdWrite { conn_handle, char_handle, notifications, indications } => {
                // Forward CCCD write event to host
                let cccd_event = events::create_cccd_write_event(
                    conn_handle,
                    char_handle,
                    notifications,
                    indications,
                );
                
                if let Err(_) = events::forward_event_to_host(cccd_event).await {
                    error!("Failed to forward CCCD write event to host");
                }
            },
            DynamicGattEvent::MtuExchange { conn_handle, mtu } => {
                info!("MTU exchanged: conn={}, mtu={}", conn_handle, mtu);
                // MTU events could be forwarded if needed
            },
        }
    }
}

impl gatt_server::Server for Server {
    type Event = DynamicGattEvent;
    
    fn on_write(
        &self,
        conn: &Connection,
        handle: u16,
        op: WriteOp,
        offset: usize,
        data: &[u8],
    ) -> Option<Self::Event> {
        // Delegate to dynamic server
        self.dynamic_server.on_write(conn, handle, op, offset, data)
    }
}

/// Server event type (re-export from dynamic_gatt)
pub type ServerEvent = DynamicGattEvent;

