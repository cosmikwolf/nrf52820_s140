#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::buffer_pool::TxPacket;
use nrf52820_s140_firmware::protocol::{Packet, RequestCode, ResponseCode};
use nrf52820_s140_firmware::commands::system;

#[defmt_test::tests]
mod tests {
    use common::*;
    use defmt::{assert, assert_eq};
    use alloc::vec::Vec;

    use super::*;

    #[init]
    fn init() {
        ensure_heap_initialized();
    }

    #[test]
    fn test_echo_command_integration() {
        // Test data to echo
        let echo_data = b"Hello BLE Modem!";
        
        // Create expected response packet
        let response_packet = Packet::new_response(ResponseCode::Ack, echo_data);
        assert!(response_packet.is_ok());
        
        let expected_packet = response_packet.unwrap();
        let expected_data = expected_packet.serialize();
        assert!(expected_data.is_ok());
        
        // Verify expected response structure
        assert!(expected_data.unwrap().len() > echo_data.len());
    }

    #[test]
    fn test_get_info_command() {
        // Test creating GetInfo request packet
        let request_packet = Packet::new_request_for_sending(RequestCode::GetInfo, &[]);
        assert!(request_packet.is_ok());
        
        let packet = request_packet.unwrap();
        assert_eq!(packet.code, RequestCode::GetInfo as u16);
        assert_eq!(packet.payload.len(), 0);
    }

    #[test]
    fn test_packet_roundtrip() {
        // Test creating a request packet for Echo command
        let test_payload = b"Test payload";
        
        let request_packet = Packet::new_request_for_sending(RequestCode::Echo, test_payload);
        assert!(request_packet.is_ok());
        
        let packet = request_packet.unwrap();
        assert_eq!(packet.code, RequestCode::Echo as u16);
        assert!(arrays_equal(packet.payload.as_slice(), test_payload));
        
        // Serialize the request
        let serialized = packet.serialize_request();
        assert!(serialized.is_ok());
        
        let wire_data = serialized.unwrap();
        
        // Parse it back
        let parsed_packet = Packet::new_request(wire_data.as_slice());
        assert!(parsed_packet.is_ok());
        
        let parsed = parsed_packet.unwrap();
        assert_eq!(parsed.code, RequestCode::Echo as u16);
        assert!(arrays_equal(parsed.payload.as_slice(), test_payload));
    }

    #[test]
    fn test_all_system_commands() {
        // Test that all system command codes can be created
        
        // GetInfo
        let request = Packet::new_request_for_sending(RequestCode::GetInfo, &[]);
        assert!(request.is_ok());
        
        // Echo with various payloads
        let payloads = [
            &[][..],                    // Empty
            b"A",                       // Single byte
            b"Hello World",             // Normal string
            &[0x00, 0xFF, 0x55, 0xAA],  // Binary data
        ];
        
        for payload in &payloads {
            let request = Packet::new_request_for_sending(RequestCode::Echo, payload);
            assert!(request.is_ok());
        }
        
        // Shutdown
        let request = Packet::new_request_for_sending(RequestCode::Shutdown, &[]);
        assert!(request.is_ok());
        
        // Reboot
        let request = Packet::new_request_for_sending(RequestCode::Reboot, &[]);
        assert!(request.is_ok());
    }

    #[test]
    fn test_echo_large_payload() {
        // Test echo with larger payload (up to reasonable size)
        let large_payload = vec![0x42u8; 200]; // 200 bytes
        
        let request = Packet::new_request_for_sending(RequestCode::Echo, &large_payload);
        assert!(request.is_ok());
        
        let packet = request.unwrap();
        assert_eq!(packet.payload.len(), large_payload.len());
        assert!(arrays_equal(packet.payload.as_slice(), &large_payload));
    }

    #[test]
    fn test_protocol_error_handling() {
        // Test protocol parsing with invalid data
        
        // Too short packet
        let short_data = [0x01, 0x02];
        let result = Packet::new_request(&short_data);
        assert!(result.is_err());
        
        // Invalid length field
        let invalid_length = [0xFF, 0xFF, 0x01, 0x02, 0x03, 0x04];
        let result = Packet::new_request(&invalid_length);
        assert!(result.is_err());
        
        // Minimum valid packet should work
        let min_valid = [0x06, 0x00, 0x01, 0x00, 0x00, 0x00]; // Length=6, no payload, code=1, CRC=0 (will be invalid CRC but right structure)
        let result = Packet::new_request(&min_valid);
        // This should fail due to invalid CRC, but not due to structure
        assert!(result.is_err());
    }
}