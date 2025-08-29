#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::core::memory::{BufferError, RxBuffer, TxPacket};
use nrf52820_s140_firmware::core::protocol::{Packet, RequestCode};
use proptest::prelude::*;

#[defmt_test::tests]
mod tests {
    use common::*;
    use defmt::{assert, assert_eq, info};
    extern crate alloc;
    use alloc::format;

    use super::*;

    #[init]
    fn init() {
        info!("begin spi tests");
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
    #[test]
    fn test_buffer_size_boundaries() {
        proptest! {
            |(data in prop::collection::vec(any::<u8>(), 0..300))| {
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
    }

    #[test]
    fn test_buffer_data_integrity() {
        proptest! {
            |(data in prop::collection::vec(any::<u8>(), 0..249))| {
                let packet = TxPacket::new(&data);
                prop_assert!(packet.is_ok());

                let packet = packet.unwrap();
                prop_assert_eq!(packet.len(), data.len());
                prop_assert!(arrays_equal(packet.as_slice(), &data));
                prop_assert_eq!(packet.is_empty(), data.is_empty());
            }
        }
    }

    #[test]
    fn test_rx_buffer_length_validation() {
        proptest! {
            |(len in 0usize..400)| {
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
    }

    #[test]
    fn test_packet_serialization_roundtrip() {
        proptest! {
            |(
                request_code in prop::sample::select(vec![
                    RequestCode::GetInfo,
                    RequestCode::GapAdvStart,
                    RequestCode::GapAdvStop,
                    RequestCode::GapGetAddr,
                    RequestCode::GapSetAddr
                ]),
                payload in prop::collection::vec(any::<u8>(), 0..200)
            )| {
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
    }

    #[test]
    fn test_request_code_preservation() {
        proptest! {
            |(request_code in prop::sample::select(vec![
                RequestCode::GetInfo,
                RequestCode::Echo,
                RequestCode::Shutdown,
                RequestCode::Reboot,
                RequestCode::RegisterEventCallback,
                RequestCode::ClearEventCallbacks,
                RequestCode::RegisterUuidGroup,
                RequestCode::GapGetAddr,
                RequestCode::GapSetAddr,
                RequestCode::GapAdvStart,
                RequestCode::GapAdvStop,
                RequestCode::GapAdvSetConfigure,
                RequestCode::GapGetName,
                RequestCode::GapSetName
            ]))| {
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
    }
}
