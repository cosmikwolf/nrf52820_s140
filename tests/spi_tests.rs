#![no_std]
#![no_main]

mod common;

use nrf52820_s140_firmware::buffer_pool::{RxBuffer, TxPacket, BufferError};

#[defmt_test::tests]
mod tests {
    use super::*;
    use crate::common::*;
    use defmt::{assert, assert_eq};

    #[test]
    fn test_buffer_allocation() {
        log_test_start("buffer_allocation");
        
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
}