//! UUID Management Commands
//! 
//! Handles UUID base registration:
//! - REQ_REGISTER_UUID_GROUP: Register 128-bit vendor UUID base

use defmt::{debug, info};

use crate::{
    buffer_pool::TxPacket,
    commands::{CommandError, ResponseBuilder},
    protocol::serialization::*,
    state::{with_state, UuidBase},
};

/// Handle REGISTER_UUID_GROUP command (0x0010)
/// Registers a 128-bit vendor-specific UUID base
/// 
/// Payload format:
/// - 16 bytes: UUID base (128-bit UUID in little-endian format)
/// 
/// Response format:
/// - 1 byte: UUID base handle (0-3)
pub async fn handle_register_uuid_group(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("UUID: REGISTER_UUID_GROUP requested");
    
    if payload.len() != 16 {
        debug!("UUID: Invalid payload length: {} (expected 16)", payload.len());
        return ResponseBuilder::build_error(0x01); // Invalid length
    }

    // Extract UUID base from payload
    let mut uuid_base = [0u8; 16];
    uuid_base.copy_from_slice(payload);

    debug!("UUID: Registering UUID base: {:02X}", uuid_base);

    // Register the UUID base in state
    let handle = with_state(|state| {
        state.register_uuid_base(uuid_base)
    }).await?;

    info!("UUID: Registered UUID base with handle: {}", handle);

    // Build response with the assigned handle
    let mut response = ResponseBuilder::new();
    response.add_u8(handle)?;
    
    response.build(crate::protocol::ResponseCode::Ack)
}

/// Convert UUID base to nRF SoftDevice format
/// 
/// The nRF SoftDevice expects UUID bases to be registered in a specific format.
/// This function converts our stored UUID base to the format expected by the SoftDevice.
pub fn uuid_base_to_softdevice_format(uuid_base: &[u8; 16]) -> [u8; 16] {
    // The original C implementation stores UUID bases as-is
    // The SoftDevice expects them in little-endian format
    // For now, we'll store them as received from the host
    *uuid_base
}

/// Get UUID base by handle for SoftDevice operations
pub async fn get_uuid_base_for_softdevice(handle: u8) -> Option<[u8; 16]> {
    with_state(|state| {
        state.get_uuid_base(handle)
            .map(|base| uuid_base_to_softdevice_format(&base.base))
    }).await
}

/// Utility function to create a 128-bit UUID from base and offset
/// 
/// This is used when creating services and characteristics with vendor-specific UUIDs.
/// The 16-bit offset is inserted into the UUID base at the appropriate position.
pub fn create_uuid_from_base_and_offset(uuid_base: &[u8; 16], offset: u16) -> [u8; 16] {
    let mut result = *uuid_base;
    
    // Insert the 16-bit offset at bytes 12-13 (little-endian)
    // This matches the Nordic UUID format where the 16-bit part is at positions 12-13
    let offset_bytes = offset.to_le_bytes();
    result[12] = offset_bytes[0];
    result[13] = offset_bytes[1];
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_uuid_from_base_and_offset() {
        let base = [0x00, 0x63, 0x4C, 0xF7, 0x6F, 0x05, 0x53, 0xB6, 
                    0xFB, 0x47, 0x75, 0x9E, 0x00, 0x00, 0xC8, 0xC3];
        let offset = 0x0001;
        
        let result = create_uuid_from_base_and_offset(&base, offset);
        
        // Check that offset was inserted at bytes 12-13
        assert_eq!(result[12], 0x01);
        assert_eq!(result[13], 0x00);
        
        // Check that other bytes remain unchanged
        assert_eq!(result[0], 0x00);
        assert_eq!(result[14], 0xC8);
        assert_eq!(result[15], 0xC3);
    }
}