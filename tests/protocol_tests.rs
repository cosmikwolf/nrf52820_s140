#![no_std]
#![no_main]

mod common;

use nrf52820_s140_firmware::core::protocol::{calculate_crc16, validate_crc16};

#[defmt_test::tests]
mod tests {
    use defmt::{assert, assert_eq, assert_ne};

    use super::*;
    use crate::common::*;

    #[test]
    fn test_crc16_calculation() {
        // Test with known data
        let data = b"Hello, World!";
        let crc = calculate_crc16(data);

        // CRC16 should produce consistent results
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
    fn test_empty_data_crc() {
        // Test CRC calculation with empty data
        let empty_data = b"";
        let crc = calculate_crc16(empty_data);

        // Validation should work for empty data too
        assert!(validate_crc16(empty_data, crc));
    }

    #[test]
    fn test_known_crc_values() {
        // Test some known patterns
        let test_cases = [
            (b"123456789" as &[u8], "known test vector"),
            (b"\x00\x01\x02\x03\x04" as &[u8], "sequential bytes"),
            (b"\xFF\xFF\xFF\xFF" as &[u8], "all ones"),
            (b"A" as &[u8], "single character"),
        ];

        for (data, _description) in &test_cases {
            let crc = calculate_crc16(data);

            // Verify validation works
            assert!(validate_crc16(data, crc));

            // Verify different CRC fails
            assert!(!validate_crc16(data, crc ^ 0x1234));
        }
    }

    #[test]
    fn test_crc16_boundary_conditions() {
        // Test maximum length data (within reasonable bounds)
        let max_data = create_test_data(200, 0xAA);
        let crc1 = calculate_crc16(&max_data);
        assert!(validate_crc16(&max_data, crc1));

        // Test different patterns should give different CRCs
        let pattern2 = create_test_data(200, 0x55);
        let crc2 = calculate_crc16(&pattern2);
        assert_ne!(crc1, crc2);
    }
}

