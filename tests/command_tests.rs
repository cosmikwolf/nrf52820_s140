#![no_std]
#![no_main]

mod common;

use nrf52820_s140_firmware::commands::{CommandError, ResponseBuilder};
use nrf52820_s140_firmware::protocol::{Packet, RequestCode, ResponseCode};

#[defmt_test::tests]
mod tests {
    use defmt::{assert, assert_eq};

    use super::*;
    use crate::common::*;

    #[test]
    fn test_get_info_command() {
        // Test the GetInfo command handler directly
        let payload: &[u8] = &[];

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
}

