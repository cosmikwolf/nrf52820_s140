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
use nrf52820_s140_firmware::protocol::{Packet, RequestCode};
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
    
    #[test]
    fn test_buffer_allocation_properties() {
        log_test_start("buffer_allocation_properties");
        
        // Property: Valid sized data should always allocate successfully
        let test_sizes = [0, 1, 10, 50, 100, 200, 249]; // Valid sizes
        let invalid_sizes = [250, 300, 500]; // Invalid sizes
        
        for &size in &test_sizes {
            let test_data = create_test_data(size, 0x42);
            let result = TxPacket::new(test_data.as_slice());
            
            assert!(result.is_ok(), "Failed to allocate buffer for size {}", size);
            let packet = result.unwrap();
            assert_eq!(packet.len(), size);
            assert!(arrays_equal(packet.as_slice(), test_data.as_slice()));
            info!("✓ Buffer allocation property holds for size {}", size);
        }
        
        for &size in &invalid_sizes {
            // Create large data using alloc::vec::Vec for sizes > 256
            use alloc::vec::Vec;
            let mut large_data = Vec::new();
            for i in 0..size {
                large_data.push(0xFF_u8.wrapping_add(i as u8));
            }
            let result = TxPacket::new(&large_data);
            assert!(matches!(result, Err(BufferError::BufferTooSmall)), 
                    "Should reject oversized buffer for size {}", size);
            info!("✓ Correctly rejected oversized buffer for size {}", size);
        }
        
        log_test_pass("buffer_allocation_properties");
    }
    
    #[test] 
    fn test_rx_buffer_length_properties() {
        log_test_start("rx_buffer_length_properties");
        
        // Property: Valid lengths should succeed, invalid should fail
        let valid_lengths = [0, 1, 10, 50, 100, 200, 249];
        let invalid_lengths = [250, 300, 400, 500];
        
        for &len in &valid_lengths {
            let mut rx_buf = RxBuffer::new();
            let result = rx_buf.set_len(len);
            
            assert!(result.is_ok(), "Should accept valid length {}", len);
            assert_eq!(rx_buf.len(), len);
            assert_eq!(rx_buf.is_empty(), len == 0);
            info!("✓ RX buffer length property holds for len {}", len);
        }
        
        for &len in &invalid_lengths {
            let mut rx_buf = RxBuffer::new();
            let result = rx_buf.set_len(len);
            
            assert!(matches!(result, Err(BufferError::InvalidSize)), 
                    "Should reject invalid length {}", len);
            info!("✓ Correctly rejected invalid length {}", len);
        }
        
        log_test_pass("rx_buffer_length_properties");
    }
    
    #[test]
    fn test_packet_structure_properties() {
        log_test_start("packet_structure_properties");
        
        // Property: Valid packet creation and serialization should roundtrip correctly
        let test_cases = [
            (RequestCode::GetInfo, &[] as &[u8]),
            (RequestCode::GapAdvStart, &[0x01, 0x00]), // handle, conn_cfg_tag
            (RequestCode::GapAdvStop, &[0x01]), // handle
            (RequestCode::GapGetAddr, &[]),
            (RequestCode::GapSetAddr, &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x00]), // addr + type
        ];
        
        for (request_code, payload) in &test_cases {
            // Test packet creation
            let packet = Packet::new_request_for_sending(*request_code, payload);
            assert!(packet.is_ok(), "Failed to create packet for {:?}", request_code);
            
            let packet = packet.unwrap();
            assert_eq!(packet.code, *request_code as u16);
            assert!(arrays_equal(packet.payload.as_slice(), payload));
            
            // Test serialization roundtrip
            let serialized = packet.serialize_request();
            assert!(serialized.is_ok(), "Failed to serialize packet for {:?}", request_code);
            
            let wire_data = serialized.unwrap();
            info!("Serialized {:?}: {} bytes", request_code, wire_data.len());
            
            // Test parsing back
            let parsed = Packet::new_request(wire_data.as_slice());
            assert!(parsed.is_ok(), "Failed to parse serialized packet for {:?}", request_code);
            
            let parsed_packet = parsed.unwrap();
            assert_eq!(parsed_packet.code, *request_code as u16);
            assert!(arrays_equal(parsed_packet.payload.as_slice(), payload));
            
            info!("✓ Packet structure property holds for {:?}", request_code);
        }
        
        log_test_pass("packet_structure_properties");
    }
}