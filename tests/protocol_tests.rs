#![no_std]
#![no_main]

mod common;

use nrf52820_s140_firmware::protocol::{
    calculate_crc16, validate_crc16
};

#[defmt_test::tests]
mod tests {
    use super::*;
    use crate::common::*;
    use defmt::{assert, assert_eq, assert_ne};

    #[test]
    fn test_crc16_calculation() {
        log_test_start("crc16_calculation");
        
        // Test with known data
        let data = b"Hello, World!";
        let crc = calculate_crc16(data);
        info!("CRC16 of 'Hello, World!' = 0x{:04X}", crc);
        
        // CRC16 should produce consistent results
        assert_ne!(crc, 0); // Should not be zero for this data
        
        // Test that same data produces same CRC
        let crc2 = calculate_crc16(data);
        assert_eq!(crc, crc2);
        
        // Test that different data produces different CRC
        let crc3 = calculate_crc16(b"Hello, World?");
        assert_ne!(crc, crc3);
        
        log_test_pass("crc16_calculation");
    }

    #[test]
    fn test_crc16_validation() {
        log_test_start("crc16_validation");
        
        let data = b"Test message";
        let crc = calculate_crc16(data);
        info!("CRC16 of 'Test message' = 0x{:04X}", crc);
        
        // Valid CRC should pass validation
        assert!(validate_crc16(data, crc));
        
        // Invalid CRC should fail validation
        assert!(!validate_crc16(data, crc + 1));
        assert!(!validate_crc16(data, 0));
        
        log_test_pass("crc16_validation");
    }

    #[test]
    fn test_empty_data_crc() {
        log_test_start("empty_data_crc");
        
        // Test CRC calculation with empty data
        let empty_data = b"";
        let crc = calculate_crc16(empty_data);
        info!("CRC16 of empty data = 0x{:04X}", crc);
        
        // Validation should work for empty data too
        assert!(validate_crc16(empty_data, crc));
        
        log_test_pass("empty_data_crc");
    }

    #[test]
    fn test_known_crc_values() {
        log_test_start("known_crc_values");
        
        // Test some known patterns
        let test_cases = [
            (b"123456789" as &[u8], "known test vector"),
            (b"\x00\x01\x02\x03\x04" as &[u8], "sequential bytes"),
            (b"\xFF\xFF\xFF\xFF" as &[u8], "all ones"),
            (b"A" as &[u8], "single character"),
        ];
        
        for (data, description) in &test_cases {
            let crc = calculate_crc16(data);
            info!("CRC16 of {} = 0x{:04X}", description, crc);
            
            // Verify validation works
            assert!(validate_crc16(data, crc));
            
            // Verify different CRC fails
            assert!(!validate_crc16(data, crc ^ 0x1234));
        }
        
        log_test_pass("known_crc_values");
    }

    #[test]
    fn test_crc16_boundary_conditions() {
        log_test_start("crc16_boundary_conditions");
        
        // Test maximum length data (within reasonable bounds)
        let max_data = create_test_data(200, 0xAA);
        let crc1 = calculate_crc16(&max_data);
        info!("CRC16 of 200-byte pattern = 0x{:04X}", crc1);
        assert!(validate_crc16(&max_data, crc1));
        
        // Test different patterns should give different CRCs
        let pattern2 = create_test_data(200, 0x55);
        let crc2 = calculate_crc16(&pattern2);
        assert_ne!(crc1, crc2);
        
        log_test_pass("crc16_boundary_conditions");
    }
}