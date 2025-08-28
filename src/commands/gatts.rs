//! GATTS (GATT Server) Commands Implementation
//! 
//! Handles GATT Server operations including service creation,
//! characteristic management, and data operations.

use defmt::{debug, info, warn, error};
use nrf_softdevice::Softdevice;
use nrf_softdevice::ble::gatt_server::builder::ServiceBuilder;

use crate::{
    buffer_pool::TxPacket,
    commands::{CommandError, ResponseBuilder},
    protocol::serialization::PayloadReader,
};
use crate::gatt_registry::{with_registry, BleUuid, ServiceType};

/// Handle GATTS_SERVICE_ADD command (0x0080)
/// 
/// Payload format:
/// [UUID Type (1)] [UUID (2 or 16 bytes)] [Service Type (1)]
/// 
/// Response format:
/// [Service Handle (2)]
pub async fn handle_service_add(payload: &[u8], sd: &Softdevice) -> Result<TxPacket, CommandError> {
    debug!("GATTS: SERVICE_ADD requested");
    
    if payload.len() < 4 {
        debug!("GATTS: Invalid payload length: {} (expected >= 4)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    
    // Parse UUID type
    let uuid_type = reader.read_u8()?;
    debug!("GATTS: UUID type: {}", uuid_type);
    
    // Parse UUID based on type
    let uuid_data = match uuid_type {
        0 => {
            // 16-bit UUID
            if reader.remaining() < 2 {
                return ResponseBuilder::build_error(CommandError::InvalidPayload);
            }
            &payload[1..3]
        },
        1 => {
            // 128-bit UUID
            if reader.remaining() < 16 {
                return ResponseBuilder::build_error(CommandError::InvalidPayload);
            }
            &payload[1..17]
        },
        2 => {
            // Vendor-specific UUID (base_id + offset)
            if reader.remaining() < 3 {
                return ResponseBuilder::build_error(CommandError::InvalidPayload);
            }
            &payload[1..4]
        },
        _ => {
            debug!("GATTS: Invalid UUID type: {}", uuid_type);
            return ResponseBuilder::build_error(CommandError::InvalidPayload);
        }
    };
    
    // Parse service type (after UUID data)
    let service_type_offset = 1 + uuid_data.len();
    if payload.len() <= service_type_offset {
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }
    
    let service_type_byte = payload[service_type_offset];
    let service_type = match service_type_byte {
        1 => ServiceType::Primary,
        2 => ServiceType::Secondary,
        _ => {
            debug!("GATTS: Invalid service type: {}", service_type_byte);
            return ResponseBuilder::build_error(CommandError::InvalidPayload);
        }
    };
    
    debug!("GATTS: Service type: {:?}", service_type);
    
    // Parse UUID
    let ble_uuid = match BleUuid::from_payload(uuid_type, uuid_data) {
        Ok(uuid) => uuid,
        Err(_) => {
            debug!("GATTS: Failed to parse UUID");
            return ResponseBuilder::build_error(CommandError::InvalidPayload);
        }
    };
    
    debug!("GATTS: Parsed UUID: {:?}", ble_uuid);
    
    // Convert to nrf-softdevice UUID
    let softdevice_uuid = with_registry(|registry| {
        ble_uuid.to_softdevice_uuid(registry)
    });
    
    let uuid = match softdevice_uuid {
        Some(uuid) => uuid,
        None => {
            error!("GATTS: Failed to convert UUID (missing UUID base?)");
            return ResponseBuilder::build_error(CommandError::InvalidPayload);
        }
    };
    
    // Create the service using ServiceBuilder
    // Note: We need a mutable reference to Softdevice, but we only have &Softdevice
    // This is a limitation - we'll need to restructure to get &mut Softdevice
    // For now, return an error indicating this needs to be implemented
    
    warn!("GATTS: Service creation requires mutable Softdevice reference - architecture needs update");
    
    // Placeholder response with dummy handle for now
    let service_handle = 0x0001u16;
    
    info!("GATTS: Would create service with handle: {}", service_handle);
    
    // Store in registry (if service creation succeeds)
    if let Err(e) = with_registry(|registry| {
        registry.add_service(service_handle, ble_uuid, service_type)
    }) {
        error!("GATTS: Failed to add service to registry: {:?}", e);
        return ResponseBuilder::build_error(CommandError::StateError(
            crate::state::StateError::ServicesExhausted
        ));
    }
    
    // Build response with service handle
    let mut response = ResponseBuilder::new();
    response.add_u16(service_handle)?;
    
    response.build(crate::protocol::ResponseCode::Ack)
}

/// Handle GATTS_CHARACTERISTIC_ADD command (0x0081)
/// 
/// Complex payload format (from protocol analysis):
/// [Service Handle (2)] [UUID Type (1)] [UUID Data (2-16)] [Properties (1)] 
/// [Max Length (2)] [Initial Value Length (1)] [Initial Value (0-N)]
/// [Permissions (1)] [Metadata flags indicating optional descriptors]
/// 
/// Response format:
/// [Value Handle (2)] [CCCD Handle (2)] [SCCD Handle (2)]
pub async fn handle_characteristic_add(payload: &[u8], _sd: &Softdevice) -> Result<TxPacket, CommandError> {
    debug!("GATTS: CHARACTERISTIC_ADD requested");
    
    if payload.len() < 8 {
        debug!("GATTS: Invalid payload length: {} (expected >= 8)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    
    // Parse service handle
    let service_handle = reader.read_u16()?;
    debug!("GATTS: Service handle: {}", service_handle);
    
    // Verify service exists
    let service_exists = with_registry(|registry| {
        registry.find_service(service_handle).is_some()
    });
    
    if !service_exists {
        debug!("GATTS: Service handle {} not found", service_handle);
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }
    
    // Parse UUID type
    let uuid_type = reader.read_u8()?;
    
    // Parse UUID data length based on type
    let uuid_len = match uuid_type {
        0 => 2,  // 16-bit UUID
        1 => 16, // 128-bit UUID  
        2 => 3,  // Vendor-specific (base_id + offset)
        _ => {
            debug!("GATTS: Invalid UUID type: {}", uuid_type);
            return ResponseBuilder::build_error(CommandError::InvalidPayload);
        }
    };
    
    if reader.remaining() < uuid_len {
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }
    
    let uuid_data = reader.read_slice(uuid_len)?;
    
    // Parse characteristic properties
    let properties = reader.read_u8()?;
    debug!("GATTS: Properties: 0x{:02X}", properties);
    
    // Parse max length
    let max_length = reader.read_u16()?;
    debug!("GATTS: Max length: {}", max_length);
    
    // Parse initial value
    let initial_value_len = reader.read_u8()? as usize;
    if reader.remaining() < initial_value_len {
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }
    
    let initial_value = reader.read_slice(initial_value_len)?;
    debug!("GATTS: Initial value length: {}", initial_value_len);
    
    // Parse permissions (if remaining)
    let permissions = if reader.remaining() > 0 {
        reader.read_u8()?
    } else {
        0x00 // Default permissions
    };
    
    // Parse UUID
    let ble_uuid = match BleUuid::from_payload(uuid_type, uuid_data) {
        Ok(uuid) => uuid,
        Err(_) => {
            debug!("GATTS: Failed to parse characteristic UUID");
            return ResponseBuilder::build_error(CommandError::InvalidPayload);
        }
    };
    
    debug!("GATTS: Parsed characteristic UUID: {:?}", ble_uuid);
    
    // TODO: Actually create the characteristic using ServiceBuilder
    // This requires architectural changes to maintain ServiceBuilder references
    
    // Placeholder handles for now
    let value_handle = 0x0010u16;
    let cccd_handle = if properties & 0x10 != 0 || properties & 0x20 != 0 {
        0x0011u16 // CCCD present if notify or indicate
    } else {
        0x0000u16 // No CCCD
    };
    let sccd_handle = 0x0000u16; // SCCD rarely used
    
    info!("GATTS: Would create characteristic - value: {}, CCCD: {}, SCCD: {}", 
          value_handle, cccd_handle, sccd_handle);
    
    // Store in registry
    if let Err(e) = with_registry(|registry| {
        registry.add_characteristic(
            service_handle,
            value_handle,
            cccd_handle,
            sccd_handle,
            ble_uuid,
            properties,
            max_length,
            permissions,
        )
    }) {
        error!("GATTS: Failed to add characteristic to registry: {:?}", e);
        return ResponseBuilder::build_error(CommandError::StateError(
            crate::state::StateError::CharacteristicsExhausted
        ));
    }
    
    // Build response with handles
    let mut response = ResponseBuilder::new();
    response.add_u16(value_handle)?;
    response.add_u16(cccd_handle)?; 
    response.add_u16(sccd_handle)?;
    
    response.build(crate::protocol::ResponseCode::Ack)
}

/// Handle GATTS_HVX command (0x0083) - Handle Value Notification/Indication
/// 
/// Payload format:
/// [Connection Handle (2)] [Characteristic Handle (2)] [Type (1)] [Data Length (2)] [Data (0-N)]
/// 
/// Type: 0x01 = Notification, 0x02 = Indication
pub async fn handle_hvx(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GATTS: HVX requested");
    
    if payload.len() < 7 {
        debug!("GATTS: Invalid payload length: {} (expected >= 7)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    
    let conn_handle = reader.read_u16()?;
    let char_handle = reader.read_u16()?;
    let hvx_type = reader.read_u8()?;
    let data_length = reader.read_u16()? as usize;
    
    debug!("GATTS: HVX - conn: {}, char: {}, type: {}, len: {}", 
           conn_handle, char_handle, hvx_type, data_length);
    
    if reader.remaining() < data_length {
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }
    
    let _data = reader.read_slice(data_length)?;
    
    // TODO: Implement actual notification/indication sending
    // This requires access to the Connection object, which we need to store
    // somewhere accessible from command handlers
    
    match hvx_type {
        0x01 => {
            info!("GATTS: Would send notification on handle {}", char_handle);
            // utils::send_notification(conn, char_handle, data)?;
        },
        0x02 => {
            info!("GATTS: Would send indication on handle {}", char_handle);
            // utils::send_indication(conn, char_handle, data)?;
        },
        _ => {
            debug!("GATTS: Invalid HVX type: {}", hvx_type);
            return ResponseBuilder::build_error(CommandError::InvalidPayload);
        }
    }
    
    ResponseBuilder::build_ack()
}

/// Handle GATTS_MTU_REPLY command (0x0082)
/// 
/// Payload format:
/// [Connection Handle (2)] [MTU (2)]
pub async fn handle_mtu_reply(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GATTS: MTU_REPLY requested");
    
    if payload.len() < 4 {
        debug!("GATTS: Invalid payload length: {} (expected 4)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    
    let conn_handle = reader.read_u16()?;
    let mtu = reader.read_u16()?;
    
    debug!("GATTS: MTU reply - conn: {}, MTU: {}", conn_handle, mtu);
    
    // TODO: Implement actual MTU reply
    // This requires integration with connection management
    
    info!("GATTS: Would reply to MTU exchange with MTU {}", mtu);
    
    ResponseBuilder::build_ack()
}

/// Handle GATTS_SYS_ATTR_SET command (0x0085) - System Attributes (Bonding)
/// 
/// Payload format:
/// [Connection Handle (2)] [Sys Attr Length (2)] [Sys Attr Data (0-N)]
pub async fn handle_sys_attr_set(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GATTS: SYS_ATTR_SET requested");
    
    if payload.len() < 4 {
        debug!("GATTS: Invalid payload length: {} (expected >= 4)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    
    let conn_handle = reader.read_u16()?;
    let attr_length = reader.read_u16()? as usize;
    
    debug!("GATTS: Sys attr - conn: {}, len: {}", conn_handle, attr_length);
    
    if reader.remaining() < attr_length {
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }
    
    let _sys_attr_data = reader.read_slice(attr_length)?;
    
    // TODO: Implement system attributes handling for bonding
    // This is used to restore CCCD states and other client-specific data
    
    info!("GATTS: Would set system attributes for connection {}", conn_handle);
    
    ResponseBuilder::build_ack()
}