#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

// Global allocator for proptest (required for alloc feature in no_std)
extern crate alloc;
use embedded_alloc::LlffHeap as Heap;

#[global_allocator]
static HEAP: Heap = Heap::empty();

// Define the global allocator backing store - 8KB heap for more complex tests
static mut HEAP_MEM: [u8; 8192] = [0; 8192];

use nrf52820_s140_firmware::buffer_pool::{RxBuffer, TxPacket, BufferError};
use nrf52820_s140_firmware::protocol::{Packet, RequestCode, ResponseCode, ProtocolError};
use proptest::prelude::*;
use alloc::vec::Vec;

#[defmt_test::tests]
mod tests {
    use super::*;
    use crate::common::*;
    use defmt::{assert, assert_eq};

    #[test]
    fn test_buffer_allocation() {
        log_test_start("buffer_allocation");
        
        // Initialize the heap allocator (only needed once)
        unsafe {
            crate::HEAP.init(crate::HEAP_MEM.as_ptr() as usize, crate::HEAP_MEM.len());
        }
        
        // Test successful packet allocation
        let data = b"test data";
        let packet = TxPacket::new(data);
        assert!(packet.is_ok());
        
        let packet = packet.unwrap();
        assert_eq!(packet.len(), data.len());
        assert_eq!(packet.as_slice(), data);
        assert!(!packet.is_empty());
        
        info!("Successfully allocated packet with {} bytes", packet.len());
        log_test_pass("buffer_allocation");
    }

    #[test]
    fn test_buffer_too_large() {
        log_test_start("buffer_too_large");
        
        // Test allocation failure with too large data
        let large_data = [0xFF; 250]; // Larger than BUFFER_SIZE (249)
        let result = TxPacket::new(&large_data);
        
        match result {
            Err(BufferError::BufferTooSmall) => {
                info!("Correctly rejected oversized buffer ({} bytes)", large_data.len());
            },
            _ => panic!("Expected BufferTooSmall error"),
        }
        
        log_test_pass("buffer_too_large");
    }

    #[test]
    fn test_rx_buffer_operations() {
        log_test_start("rx_buffer_operations");
        
        let mut rx_buf = RxBuffer::new();
        
        // Initially empty
        assert!(rx_buf.is_empty());
        assert_eq!(rx_buf.len(), 0);
        info!("RX buffer initialized empty");
        
        // Write some data
        let test_data = b"hello world";
        let buf_slice = rx_buf.as_mut_slice();
        buf_slice[..test_data.len()].copy_from_slice(test_data);
        rx_buf.set_len(test_data.len()).unwrap();
        
        // Check data
        assert!(!rx_buf.is_empty());
        assert_eq!(rx_buf.len(), test_data.len());
        assert!(arrays_equal(rx_buf.as_slice(), test_data));
        info!("RX buffer contains {} bytes: {:?}", rx_buf.len(), rx_buf.as_slice());
        
        // Clear buffer
        rx_buf.clear();
        assert!(rx_buf.is_empty());
        assert_eq!(rx_buf.len(), 0);
        info!("RX buffer cleared");
        
        log_test_pass("rx_buffer_operations");
    }

    #[test]
    fn test_rx_buffer_invalid_length() {
        log_test_start("rx_buffer_invalid_length");
        
        let mut rx_buf = RxBuffer::new();
        
        // Try to set length larger than buffer size
        let result = rx_buf.set_len(300); // Larger than RX_BUFFER_SIZE
        
        match result {
            Err(BufferError::InvalidSize) => {
                info!("Correctly rejected invalid length (300 bytes)");
            },
            _ => panic!("Expected InvalidSize error"),
        }
        
        log_test_pass("rx_buffer_invalid_length");
    }

    #[test]
    fn test_empty_packet() {
        log_test_start("empty_packet");
        
        // Test creating empty packet
        let empty_data = b"";
        let packet = TxPacket::new(empty_data);
        assert!(packet.is_ok());
        
        let packet = packet.unwrap();
        assert_eq!(packet.len(), 0);
        assert!(packet.is_empty());
        info!("Successfully created empty packet");
        
        log_test_pass("empty_packet");
    }

    #[test]
    fn test_buffer_patterns() {
        log_test_start("buffer_patterns");
        
        // Test various data patterns
        let patterns = [
            (b"A" as &[u8], "single byte"),
            (b"Hello, World!" as &[u8], "text data"),
            (&[0x00, 0x01, 0x02, 0x03, 0x04], "sequential bytes"),
            (&[0xFF; 10], "all ones"),
            (&[0x00; 10], "all zeros"),
        ];
        
        for (data, description) in &patterns {
            let packet = TxPacket::new(data).unwrap();
            assert_eq!(packet.len(), data.len());
            assert!(arrays_equal(packet.as_slice(), data));
            info!("Pattern '{}': {} bytes", description, packet.len());
        }
        
        log_test_pass("buffer_patterns");
    }
    
    #[test]
    fn test_command_processing_gap_commands() {
        log_test_start("command_processing_gap_commands");
        
        // Test GAP ADV_START command processing
        let mut rx_buf = RxBuffer::new();
        let adv_start_cmd = [
            0x01,       // Packet type: Command
            0x10,       // Opcode: ADV_START (GAP class 0x01, command 0x00)
            0x00,
            0x04,       // Length: 4 bytes
            0x00,
            0x01,       // Handle: 1
            0x00,       // Conn cfg tag: 0
            0x00,       // Reserved
            0x00,
        ];
        
        // Write command to RX buffer
        let buf_slice = rx_buf.as_mut_slice();
        buf_slice[..adv_start_cmd.len()].copy_from_slice(&adv_start_cmd);
        rx_buf.set_len(adv_start_cmd.len()).unwrap();
        
        // Create a valid GAP ADV_START command packet
        // Format: [Length:2][Payload:N][RequestCode:2][CRC16:2]
        let payload = [0x01, 0x00]; // Handle: 1, Conn cfg tag: 0
        let packet = Packet::new_request_for_sending(RequestCode::GapAdvStart, &payload);
        assert!(packet.is_ok());
        
        let packet = packet.unwrap();
        assert_eq!(packet.code, RequestCode::GapAdvStart as u16);
        assert!(arrays_equal(packet.payload.as_slice(), &payload));
        
        // Test serialization to wire format
        let serialized = packet.serialize_request();
        assert!(serialized.is_ok());
        
        let wire_data = serialized.unwrap();
        info!("Serialized GAP ADV_START command: {} bytes", wire_data.len());
        
        // Test parsing the serialized data back
        let parsed = Packet::new_request(wire_data.as_slice());
        match parsed {
            Ok(parsed_packet) => {
                assert_eq!(parsed_packet.code, RequestCode::GapAdvStart as u16);
                assert!(arrays_equal(parsed_packet.payload.as_slice(), &payload));
                info!("Successfully round-trip parsed GAP ADV_START command");
            },
            Err(e) => {
                panic!("Failed to parse serialized GAP command: {:?}", e);
            }
        }
        
        log_test_pass("command_processing_gap_commands");
    }
    
    proptest! {
        #[test]
        fn test_buffer_allocation_properties(data in prop::collection::vec(any::<u8>(), 0..200)) {
            // Property: Valid sized data should always allocate successfully
            let result = TxPacket::new(&data);
            
            if data.len() <= 249 { // BUFFER_SIZE
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
        fn test_rx_buffer_length_properties(len in 0usize..400) {
            // Property: Valid lengths should succeed, invalid should fail
            let mut rx_buf = RxBuffer::new();
            let result = rx_buf.set_len(len);
            
            if len <= 249 { // RX_BUFFER_SIZE
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
        fn test_command_packet_structure_properties(
            packet_type in 0u8..=3,
            opcode in any::<u16>(),
            payload_len in 0u16..=200
        ) {
            // Property: Valid command packet structure should parse correctly
            let mut packet_data = Vec::new();
            packet_data.push(packet_type);
            packet_data.extend_from_slice(&opcode.to_le_bytes());
            packet_data.extend_from_slice(&payload_len.to_le_bytes());
            
            // Add payload bytes
            for i in 0..payload_len {
                packet_data.push((i % 256) as u8);
            }
            
            if packet_type <= 2 && packet_data.len() <= 249 {
                // Valid packet type and size - should parse
                let result = Command::parse(&packet_data);
                prop_assert!(result.is_ok());
                
                let cmd = result.unwrap();
                prop_assert_eq!(cmd.opcode(), opcode);
                prop_assert_eq!(cmd.payload().len(), payload_len as usize);
            }
        }
    }
}