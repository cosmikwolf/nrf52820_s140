//! BLE Event Forwarding System
//!
//! This module captures BLE connection and GATT server events and forwards them
//! to the host via SPI communication using the BLE modem protocol.
//!
//! Based on nrf-softdevice patterns, events are handled through:
//! - Connection events from peripheral::advertise_connectable()
//! - GATT server events from gatt_server::run()

use defmt::debug;
use heapless::Vec;
use nrf_softdevice::ble::Connection;

use crate::core::{
    memory::TxPacket,
    protocol::{ResponseCode, Packet, MAX_PAYLOAD_SIZE},
    transport,
};

/// Event serialization buffer
type EventBuffer = Vec<u8, MAX_PAYLOAD_SIZE>;

/// BLE event types we forward to the host
#[derive(Debug)]
pub enum BleModemEvent {
    Connected {
        conn_handle: u16,
        peer_addr: [u8; 6],
        addr_type: u8,
    },
    Disconnected {
        conn_handle: u16,
        reason: u8,
    },
    GattsWrite {
        conn_handle: u16,
        char_handle: u16,
        data: Vec<u8, 64>,
    },
    GattsRead {
        conn_handle: u16,
        char_handle: u16,
    },
    MtuExchange {
        conn_handle: u16,
        client_mtu: u16,
        server_mtu: u16,
    },
    CccdWrite {
        conn_handle: u16,
        char_handle: u16,
        notifications: bool,
        indications: bool,
    },
}

impl BleModemEvent {
    /// Serialize event to wire format for transmission to host
    pub fn serialize(&self) -> Result<EventBuffer, ()> {
        let mut buffer = EventBuffer::new();
        
        match self {
            BleModemEvent::Connected { conn_handle, peer_addr, addr_type } => {
                // Event type: BLE_GAP_EVT_CONNECTED (0x11)
                buffer.extend_from_slice(&[0x11, 0x00]).map_err(|_| ())?;
                
                // Connection handle
                buffer.extend_from_slice(&conn_handle.to_le_bytes()).map_err(|_| ())?;
                
                // Peer address type and address
                buffer.push(*addr_type).map_err(|_| ())?;
                buffer.extend_from_slice(peer_addr).map_err(|_| ())?;
            },
            
            BleModemEvent::Disconnected { conn_handle, reason } => {
                // Event type: BLE_GAP_EVT_DISCONNECTED (0x12)
                buffer.extend_from_slice(&[0x12, 0x00]).map_err(|_| ())?;
                buffer.extend_from_slice(&conn_handle.to_le_bytes()).map_err(|_| ())?;
                buffer.push(*reason).map_err(|_| ())?;
            },
            
            BleModemEvent::GattsWrite { conn_handle, char_handle, data } => {
                // Event type: BLE_GATTS_EVT_WRITE (0x50)
                buffer.extend_from_slice(&[0x50, 0x00]).map_err(|_| ())?;
                buffer.extend_from_slice(&conn_handle.to_le_bytes()).map_err(|_| ())?;
                buffer.extend_from_slice(&char_handle.to_le_bytes()).map_err(|_| ())?;
                buffer.push(data.len() as u8).map_err(|_| ())?;
                buffer.extend_from_slice(data).map_err(|_| ())?;
            },
            
            BleModemEvent::GattsRead { conn_handle, char_handle } => {
                // Event type: BLE_GATTS_EVT_READ (0x51)
                buffer.extend_from_slice(&[0x51, 0x00]).map_err(|_| ())?;
                buffer.extend_from_slice(&conn_handle.to_le_bytes()).map_err(|_| ())?;
                buffer.extend_from_slice(&char_handle.to_le_bytes()).map_err(|_| ())?;
            },
            
            BleModemEvent::MtuExchange { conn_handle, client_mtu, server_mtu } => {
                // Event type: BLE_GATTS_EVT_EXCHANGE_MTU_REQUEST (0x52)
                buffer.extend_from_slice(&[0x52, 0x00]).map_err(|_| ())?;
                buffer.extend_from_slice(&conn_handle.to_le_bytes()).map_err(|_| ())?;
                buffer.extend_from_slice(&client_mtu.to_le_bytes()).map_err(|_| ())?;
                buffer.extend_from_slice(&server_mtu.to_le_bytes()).map_err(|_| ())?;
            },
            
            BleModemEvent::CccdWrite { conn_handle, char_handle, notifications, indications } => {
                // Event type: BLE_GATTS_EVT_CCCD_WRITE (0x53)
                buffer.extend_from_slice(&[0x53, 0x00]).map_err(|_| ())?;
                buffer.extend_from_slice(&conn_handle.to_le_bytes()).map_err(|_| ())?;
                buffer.extend_from_slice(&char_handle.to_le_bytes()).map_err(|_| ())?;
                let cccd_value = (*notifications as u8) | ((*indications as u8) << 1);
                buffer.push(cccd_value).map_err(|_| ())?;
            },
        }
        
        Ok(buffer)
    }
}

/// Forward a BLE event to the host via SPI
pub async fn forward_event_to_host(event: BleModemEvent) -> Result<(), ()> {
    // Serialize the event
    let event_data = event.serialize()?;
    
    // Create response packet with BLE event code
    let packet = Packet::new_response(ResponseCode::BleEvent, &event_data)
        .map_err(|_| ())?;
    
    // Serialize packet for transmission
    let serialized = packet.serialize().map_err(|_| ())?;
    
    // Create TX packet
    let tx_packet = TxPacket::new(&serialized).map_err(|_| ())?;
    
    // Send via SPI
    transport::send_response(tx_packet).await.map_err(|_| ())?;
    
    debug!("Event forwarded to host successfully");
    Ok(())
}

/// Create a Connected event from nrf-softdevice Connection
pub fn create_connected_event(conn: &Connection) -> BleModemEvent {
    // Note: nrf-softdevice Connection doesn't directly expose peer address
    // For now, use placeholder values. Real implementation would need to store
    // connection info during the advertising/connection process
    let conn_handle = conn.handle().unwrap_or(0);
    BleModemEvent::Connected {
        conn_handle,
        peer_addr: [0; 6], // Placeholder - would need peer address from connection
        addr_type: 0,      // Placeholder - would need address type
    }
}

/// Create a Disconnected event 
pub fn create_disconnected_event(conn_handle: u16, reason: u8) -> BleModemEvent {
    BleModemEvent::Disconnected { conn_handle, reason }
}

/// Create a GATT Write event
pub fn create_gatts_write_event(
    conn_handle: u16, 
    char_handle: u16, 
    data: &[u8]
) -> Result<BleModemEvent, ()> {
    let mut event_data = Vec::new();
    event_data.extend_from_slice(data).map_err(|_| ())?;
    
    Ok(BleModemEvent::GattsWrite {
        conn_handle,
        char_handle,
        data: event_data,
    })
}

/// Create a CCCD Write event
pub fn create_cccd_write_event(
    conn_handle: u16,
    char_handle: u16,
    notifications: bool,
    indications: bool,
) -> BleModemEvent {
    BleModemEvent::CccdWrite {
        conn_handle,
        char_handle,
        notifications,
        indications,
    }
}