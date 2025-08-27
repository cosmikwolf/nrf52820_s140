//! Unit tests for protocol layer
//! 
//! These tests run on the target hardware using defmt-test

#![no_std]
#![no_main]

use defmt_rtt as _; 
use panic_probe as _;

// Import the modules we want to test
use nrf52820_s140_firmware::protocol::{
    calculate_crc16, validate_crc16, Packet, RequestCode, ResponseCode, ProtocolError
};

#[defmt_test::tests]
mod tests {
    use super::*;
    use defmt::{assert_eq, assert, assert_ne};
    use heapless::Vec;

    #[test]
    fn test_crc16_calculation() {
        // Test with known data
        let data = b"Hello, World!";
        let crc = calculate_crc16(data);
        
        // CRC16-IBM-SDLC should produce consistent results
        assert_ne!(crc, 0); // Should not be zero for this data
        
        // Test that same data produces same CRC
        let crc2 = calculate_crc16(data);
        assert_eq!(crc, crc2);
        
        // Test that different data produces different CRC
        let crc3 = calculate_crc16(b"Hello, World?");
        assert_ne!(crc, crc3);
    }

    #[test]
    fn test_crc16_validation() {
        let data = b"Test message";
        let crc = calculate_crc16(data);
        
        // Valid CRC should pass validation
        assert!(validate_crc16(data, crc));
        
        // Invalid CRC should fail validation
        assert!(!validate_crc16(data, crc + 1));
        assert!(!validate_crc16(data, 0));
    }

    #[test]
    fn test_response_packet_serialization() {
        // Create a simple response packet
        let payload = b"test data";
        let packet = Packet::new_response(ResponseCode::Ack, payload).unwrap();
        
        // Serialize it
        let serialized = packet.serialize().unwrap();
        
        // Check format: [Length:2][ResponseCode:2][Payload:N][CRC16:2]
        let expected_length = 2 + 2 + payload.len() + 2; // length + code + payload + crc
        assert_eq!(serialized.len(), expected_length);
        
        // Check length header (big-endian)
        let length_bytes = &serialized[0..2];
        let length = u16::from_be_bytes([length_bytes[0], length_bytes[1]]);
        assert_eq!(length as usize, expected_length);
        
        // Check response code (big-endian)
        let code_bytes = &serialized[2..4];
        let code = u16::from_be_bytes([code_bytes[0], code_bytes[1]]);
        assert_eq!(code, ResponseCode::Ack as u16);
        
        // Check payload
        let payload_bytes = &serialized[4..4 + payload.len()];
        assert_eq!(payload_bytes, payload);
        
        // Check CRC is present (last 2 bytes)
        let crc_offset = serialized.len() - 2;
        let _crc_bytes = &serialized[crc_offset..];
        
        // Validate CRC over message without CRC
        let message_without_crc = &serialized[..crc_offset];
        let received_crc = u16::from_be_bytes([serialized[crc_offset], serialized[crc_offset + 1]]);
        assert!(validate_crc16(message_without_crc, received_crc));
    }

    #[test]
    fn test_request_packet_parsing() {
        // Create a request packet manually: [Length:2][Payload:N][RequestCode:2][CRC16:2]
        let payload = b"test";
        let request_code = RequestCode::GetInfo as u16;
        
        // Build message manually
        let mut message = Vec::new();
        
        // Length (payload + code + CRC = 4 + 2 + 2 = 8, plus length header = 10 total)
        let total_length = 2 + payload.len() + 2 + 2;
        message.extend_from_slice(&(total_length as u16).to_be_bytes());
        
        // Payload
        message.extend_from_slice(payload);
        
        // Request code
        message.extend_from_slice(&request_code.to_be_bytes());
        
        // Calculate CRC over everything except CRC itself
        let crc = calculate_crc16(&message);
        message.extend_from_slice(&crc.to_be_bytes());
        
        // Parse it back
        let packet = Packet::new_request(&message).unwrap();
        
        // Verify
        assert_eq!(packet.code, request_code);
        assert_eq!(packet.payload.as_slice(), payload);
        assert_eq!(packet.request_code(), Some(RequestCode::GetInfo));
    }

    #[test]
    fn test_request_packet_crc_validation() {
        // Create a valid request packet
        let payload = b"test";
        let request_code = RequestCode::GetInfo as u16;
        
        let mut message = Vec::new();
        let total_length = 2 + payload.len() + 2 + 2;
        message.extend_from_slice(&(total_length as u16).to_be_bytes());
        message.extend_from_slice(payload);
        message.extend_from_slice(&request_code.to_be_bytes());
        let crc = calculate_crc16(&message);
        message.extend_from_slice(&crc.to_be_bytes());
        
        // Valid packet should parse successfully
        assert!(Packet::new_request(&message).is_ok());
        
        // Corrupt the CRC
        let mut corrupted = message.clone();
        let crc_offset = corrupted.len() - 2;
        corrupted[crc_offset] ^= 0xFF; // Flip bits in first CRC byte
        
        // Corrupted packet should fail with CRC error
        match Packet::new_request(&corrupted) {
            Err(ProtocolError::InvalidCrc) => (), // Expected
            other => panic!("Expected InvalidCrc error, got: {:?}", other),
        }
    }

    #[test]
    fn test_request_packet_length_validation() {
        // Too short packet (less than minimum 6 bytes)
        let short_packet = [0x00, 0x04, 0x01, 0x02]; // Only 4 bytes
        match Packet::new_request(&short_packet) {
            Err(ProtocolError::InvalidLength) => (), // Expected
            other => panic!("Expected InvalidLength error, got: {:?}", other),
        }
        
        // Length mismatch
        let mut message = Vec::new();
        message.extend_from_slice(&10u16.to_be_bytes()); // Claims 10 bytes
        message.extend_from_slice(b"test"); // payload
        message.extend_from_slice(&(RequestCode::GetInfo as u16).to_be_bytes());
        let crc = calculate_crc16(&message);
        message.extend_from_slice(&crc.to_be_bytes());
        // But message is actually longer than 10 bytes
        
        match Packet::new_request(&message) {
            Err(ProtocolError::InvalidLength) => (), // Expected  
            other => panic!("Expected InvalidLength error, got: {:?}", other),
        }
    }

    #[test]
    fn test_request_for_sending() {
        // Test creating request packets for transmission (used in tests)
        let payload = b"echo test";
        let packet = Packet::new_request_for_sending(RequestCode::GetInfo, payload).unwrap();
        
        assert_eq!(packet.code, RequestCode::GetInfo as u16);
        assert_eq!(packet.payload.as_slice(), payload);
        
        // Test serialization
        let serialized = packet.serialize_request().unwrap();
        
        // Verify it can be parsed back
        let parsed = Packet::new_request(&serialized).unwrap();
        assert_eq!(parsed.code, RequestCode::GetInfo as u16);
        assert_eq!(parsed.payload.as_slice(), payload);
    }

    #[test]
    fn test_empty_payload() {
        // Test with empty payload
        let packet = Packet::new_response(ResponseCode::Ack, &[]).unwrap();
        let serialized = packet.serialize().unwrap();
        
        // Should be: [Length:2][Code:2][CRC:2] = 6 bytes total
        assert_eq!(serialized.len(), 6);
        
        // Verify length header
        let length = u16::from_be_bytes([serialized[0], serialized[1]]);
        assert_eq!(length, 6);
    }

    #[test]
    fn test_max_payload_size() {
        // Test with maximum payload size
        let max_payload = vec![0xAA; 243]; // Max payload for 249 byte total
        
        // Should succeed
        let packet = Packet::new_response(ResponseCode::Ack, &max_payload).unwrap();
        let serialized = packet.serialize().unwrap();
        
        // Total should be 2 + 2 + 243 + 2 = 249 bytes
        assert_eq!(serialized.len(), 249);
    }
}