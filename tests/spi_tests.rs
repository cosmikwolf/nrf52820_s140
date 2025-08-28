#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::buffer_pool::{BufferError, RxBuffer, TxPacket, TX_POOL_SIZE};
use nrf52820_s140_firmware::protocol::{Packet, RequestCode, ProtocolError};
use proptest::prelude::*;

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
    fn test_rx_buffer_operations() {
        let mut rx_buf = RxBuffer::new();

        // Initially empty
        assert!(rx_buf.is_empty());
        assert_eq!(rx_buf.len(), 0);

        // Write some data
        let test_data = b"hello world";
        let buf_slice = rx_buf.as_mut_slice();
        buf_slice[..test_data.len()].copy_from_slice(test_data);
        rx_buf.set_len(test_data.len()).unwrap();

        // Check data
        assert!(!rx_buf.is_empty());
        assert_eq!(rx_buf.len(), test_data.len());
        assert!(arrays_equal(rx_buf.as_slice(), test_data));

        // Clear buffer
        rx_buf.clear();
        assert!(rx_buf.is_empty());
        assert_eq!(rx_buf.len(), 0);
    }


    #[test]
    fn test_empty_packet() {
        // Test creating empty packet
        let empty_data = b"";
        let packet = TxPacket::new(empty_data);
        assert!(packet.is_ok());

        let packet = packet.unwrap();
        assert_eq!(packet.len(), 0);
        assert!(packet.is_empty());
    }

    // Property-based tests
    proptest! {
        #[test]
        fn test_buffer_size_boundaries(data in prop::collection::vec(any::<u8>(), 0..300)) {
            // Initialize heap on first run
            unsafe {
                common::HEAP.init(common::HEAP_MEM.as_ptr() as usize, common::HEAP_MEM.len());
            }

            let result = TxPacket::new(&data);
            
            if data.len() <= 249 {
                prop_assert!(result.is_ok());
                let packet = result.unwrap();
                prop_assert_eq!(packet.len(), data.len());
                prop_assert!(arrays_equal(packet.as_slice(), &data));
            } else {
                prop_assert!(matches!(result, Err(BufferError::BufferTooSmall)));
            }
        }
    }

    proptest! {
        #[test]
        fn test_buffer_data_integrity(data in prop::collection::vec(any::<u8>(), 0..249)) {
            let packet = TxPacket::new(&data);
            prop_assert!(packet.is_ok());
            
            let packet = packet.unwrap();
            prop_assert_eq!(packet.len(), data.len());
            prop_assert!(arrays_equal(packet.as_slice(), &data));
            prop_assert_eq!(packet.is_empty(), data.is_empty());
        }
    }

    proptest! {
        #[test]
        fn test_rx_buffer_length_validation(len in 0usize..400) {
            let mut rx_buf = RxBuffer::new();
            let result = rx_buf.set_len(len);
            
            if len <= 249 {
                prop_assert!(result.is_ok());
                prop_assert_eq!(rx_buf.len(), len);
                prop_assert_eq!(rx_buf.is_empty(), len == 0);
            } else {
                prop_assert!(matches!(result, Err(BufferError::InvalidSize)));
            }
        }
    }

    proptest! {
        #[test]
        fn test_packet_serialization_roundtrip(
            request_code in prop::sample::select(vec![
                RequestCode::GetInfo,
                RequestCode::GapAdvStart,
                RequestCode::GapAdvStop,
                RequestCode::GapGetAddr,
                RequestCode::GapSetAddr
            ]),
            payload in prop::collection::vec(any::<u8>(), 0..200)
        ) {
            let packet = Packet::new_request_for_sending(request_code, &payload);
            prop_assert!(packet.is_ok());
            
            let packet = packet.unwrap();
            prop_assert_eq!(packet.code, request_code as u16);
            prop_assert!(arrays_equal(packet.payload.as_slice(), &payload));
            
            let serialized = packet.serialize_request();
            prop_assert!(serialized.is_ok());
            
            let wire_data = serialized.unwrap();
            let parsed = Packet::new_request(wire_data.as_slice());
            prop_assert!(parsed.is_ok());
            
            let parsed_packet = parsed.unwrap();
            prop_assert_eq!(parsed_packet.code, request_code as u16);
            prop_assert!(arrays_equal(parsed_packet.payload.as_slice(), &payload));
        }
    }

    proptest! {
        #[test]
        fn test_request_code_preservation(
            request_code in prop::sample::select(vec![
                RequestCode::GetInfo,
                RequestCode::Echo,
                RequestCode::Shutdown,
                RequestCode::Reboot,
                RequestCode::RegisterUuidGroup,
                RequestCode::GapGetAddr,
                RequestCode::GapSetAddr,
                RequestCode::GapAdvStart,
                RequestCode::GapAdvStop,
                RequestCode::GapAdvSetConfigure,
                RequestCode::GapGetName,
                RequestCode::GapSetName
            ])
        ) {
            let payload = [];
            let packet = Packet::new_request_for_sending(request_code, &payload);
            prop_assert!(packet.is_ok());
            
            let packet = packet.unwrap();
            prop_assert_eq!(packet.code, request_code as u16);
            
            let serialized = packet.serialize_request();
            if let Ok(wire_data) = serialized {
                if let Ok(parsed) = Packet::new_request(wire_data.as_slice()) {
                    prop_assert_eq!(parsed.code, request_code as u16);
                }
            }
        }
    }

    #[test]
    fn test_buffer_pool_exhaustion() {
        // Property #2: Buffer Pool Exhaustion
        let mut packets = Vec::new();
        
        // Allocate TX_POOL_SIZE packets
        for _ in 0..TX_POOL_SIZE {
            let packet = TxPacket::new(&[0x42]);
            assert!(packet.is_ok());
            packets.push(packet.unwrap());
        }
        
        // Next allocation should fail
        let result = TxPacket::new(&[0x43]);
        assert!(matches!(result, Err(BufferError::PoolExhausted)));
        
        // Drop one packet and try again
        drop(packets.pop());
        let packet = TxPacket::new(&[0x44]);
        assert!(packet.is_ok());
    }

    #[test] 
    fn test_buffer_lifecycle() {
        // Property #5: Buffer Lifecycle
        let data = [0x55; 10];
        
        // Allocate and drop multiple times to verify pool reuse
        for _ in 0..3 {
            let packet = TxPacket::new(&data);
            assert!(packet.is_ok());
            let packet = packet.unwrap();
            assert!(arrays_equal(packet.as_slice(), &data));
            // Packet drops here
        }
        
        // Should still be able to allocate after multiple drops
        let packet = TxPacket::new(&data);
        assert!(packet.is_ok());
    }

    #[test]
    fn test_crc_validation() {
        // Property #8: CRC Validation
        let payload = [0x01, 0x02];
        let packet = Packet::new_request_for_sending(RequestCode::GetInfo, &payload).unwrap();
        let mut wire_data = packet.serialize_request().unwrap();
        
        // Corrupt the CRC (last 2 bytes)
        let crc_offset = wire_data.len() - 2;
        wire_data[crc_offset] ^= 0xFF;
        
        let result = Packet::new_request(wire_data.as_slice());
        assert!(matches!(result, Err(ProtocolError::InvalidCrc)));
    }

    #[test]
    fn test_length_field_consistency() {
        // Property #9: Length Field Consistency
        let payload = [0x01, 0x02];
        let packet = Packet::new_request_for_sending(RequestCode::GetInfo, &payload).unwrap();
        let mut wire_data = packet.serialize_request().unwrap();
        
        // Corrupt length field (first 2 bytes)
        wire_data[0] = 0xFF;
        wire_data[1] = 0xFF;
        
        let result = Packet::new_request(wire_data.as_slice());
        assert!(matches!(result, Err(ProtocolError::InvalidLength)));
    }

    #[test]
    fn test_minimum_packet_size() {
        // Property #10: Minimum Packet Size
        for size in 0..6 {
            let data = vec![0u8; size];
            let result = Packet::new_request(data.as_slice());
            assert!(matches!(result, Err(ProtocolError::InvalidLength)));
        }
    }

    #[test]
    fn test_maximum_payload_size() {
        // Property #11: Maximum Payload Size
        let large_payload = vec![0u8; 250]; // Larger than MAX_PAYLOAD_SIZE (249)
        let packet = Packet::new_request_for_sending(RequestCode::GetInfo, &large_payload);
        
        if let Ok(packet) = packet {
            let result = packet.serialize_request();
            assert!(matches!(result, Err(ProtocolError::BufferFull)));
        }
    }

    #[test]
    fn test_error_propagation() {
        // Property #16: Error Propagation - test From trait implementations
        use nrf52820_s140_firmware::spi_comm::SpiError;
        
        let buffer_error = BufferError::PoolExhausted;
        let spi_error = SpiError::from(buffer_error);
        assert!(matches!(spi_error, SpiError::BufferError(BufferError::PoolExhausted)));
        
        let protocol_error = ProtocolError::InvalidCrc;
        let spi_error = SpiError::from(protocol_error);
        assert!(matches!(spi_error, SpiError::ProtocolError(ProtocolError::InvalidCrc)));
    }
}

