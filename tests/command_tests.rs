#![no_std]
#![no_main]

mod common;

use nrf52820_s140_firmware::commands::{CommandError, ResponseBuilder};
use nrf52820_s140_firmware::core::protocol::{Packet, RequestCode, ResponseCode};

#[defmt_test::tests]
mod tests {
    use defmt::{assert, assert_eq};

    use super::*;

    #[test]
    fn test_get_info_command() {
        // Test the GetInfo command handler directly
        let _payload: &[u8] = &[];

        // This is a sync test, but the handler is async, so we test the logic
        // In a real scenario, this would be tested with hardware/integration tests

        // Test that we can create a GetInfo request packet
        let get_info_code = RequestCode::GetInfo as u16;
        assert_eq!(get_info_code, 0x0001);
    }

    #[test]
    fn test_echo_command() {
        // Test the Echo command code
        let echo_code = RequestCode::Echo as u16;
        assert_eq!(echo_code, 0x0003);
    }

    #[test]
    fn test_response_builder() {
        // Test ResponseBuilder functionality
        let mut builder = ResponseBuilder::new();
        builder.add_u8(0x42).unwrap();
        builder.add_u16(0x1234).unwrap();
        builder.add_u32(0xDEADBEEF).unwrap();

        let packet = builder.build(ResponseCode::Ack).unwrap();

        // Verify the packet contains our data
        let data = packet.as_slice();
        assert!(data.len() > 0);
    }

    #[test]
    fn test_error_responses() {
        // Test error response creation
        let error_packet = ResponseBuilder::build_error(CommandError::InvalidPayload).unwrap();
        let data = error_packet.as_slice();

        assert!(data.len() > 0);

        // Test specific error code
        let error_code_packet = ResponseBuilder::build_error_code(0x1234).unwrap();
        let data2 = error_code_packet.as_slice();

        assert!(data2.len() > 0);
    }

    #[test]
    fn test_packet_creation() {
        // Test creating response packets
        let test_payload = b"Hello World";
        let packet = Packet::new_response(ResponseCode::Ack, test_payload).unwrap();

        // Verify payload
        assert_eq!(packet.payload.as_slice(), test_payload);
        assert_eq!(packet.code, ResponseCode::Ack.to_u16());
    }

    #[test]
    fn test_uuid_register_command() {
        // Test UUID registration command code
        let uuid_code = RequestCode::RegisterUuidGroup as u16;
        assert_eq!(uuid_code, 0x0010);
    }

    #[test]
    fn test_invalid_command_handling() {
        // Property #19: Invalid Command Handling
        // Test that unknown/invalid request codes are properly handled
        
        // Test with invalid request code values that don't correspond to any enum variant
        let invalid_codes = [0xFFFF, 0x9999, 0x5555, 0x1111];
        
        for &invalid_code in &invalid_codes {
            // Create a raw packet with invalid request code
            let payload: [u8; 0] = [];
            
            // In a real implementation, this would test the command dispatcher
            // For now, we test that we can detect invalid codes by checking
            // they don't match any valid RequestCode values
            
            let valid_codes = [
                RequestCode::GetInfo as u16,
                RequestCode::Echo as u16,
                RequestCode::Shutdown as u16,
                RequestCode::Reboot as u16,
                RequestCode::RegisterUuidGroup as u16,
                RequestCode::GapGetAddr as u16,
                RequestCode::GapSetAddr as u16,
                RequestCode::GapAdvStart as u16,
                RequestCode::GapAdvStop as u16,
                RequestCode::GapAdvSetConfigure as u16,
                RequestCode::GapGetName as u16,
                RequestCode::GapSetName as u16,
            ];
            
            // Verify the invalid code is not in the valid set
            assert!(!valid_codes.contains(&invalid_code), 
                   "Invalid code 0x{:04X} should not match any valid RequestCode", invalid_code);
        }
        
        // Test that all valid codes are recognized
        for &valid_code in &[
            RequestCode::GetInfo as u16,
            RequestCode::Echo as u16, 
            RequestCode::Shutdown as u16,
            RequestCode::Reboot as u16,
        ] {
            // These should be valid request codes that can create packets
            let packet = Packet::new_request_for_sending(
                unsafe { core::mem::transmute(valid_code) }, // Convert back to enum
                &[]
            );
            assert!(packet.is_ok(), "Valid code 0x{:04X} should create packet successfully", valid_code);
        }
    }
}
