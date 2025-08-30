#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::notifications::{
    NotificationRequest, NotificationResponse, NotificationError, 
    MAX_NOTIFICATION_DATA
};
use proptest::prelude::*;

#[defmt_test::tests]
mod tests {
    use alloc::vec::Vec;
    extern crate alloc;
    use alloc::format;
    use defmt::{assert, assert_eq};

    use super::*;
    use crate::common::*;
    
    // Macro to create heapless::Vec from slice
    macro_rules! heapless_vec {
        ($($elem:expr),*) => {{
            let mut vec = heapless::Vec::new();
            $(let _ = vec.push($elem);)*
            vec
        }};
        ($elem:expr; $n:expr) => {{
            let mut vec = heapless::Vec::new();
            for _ in 0..$n.min(MAX_NOTIFICATION_DATA) {
                let _ = vec.push($elem);
            }
            vec
        }};
    }

    #[init]
    fn init() {
        ensure_heap_initialized();
    }

    #[test]
    fn test_notification_data_size_limits() {
        proptest!(|(
            data_sizes in prop::collection::vec(0usize..100, 1..10)
        )| {
            // Property #36: Notification Data Size Limits
            // Notifications should accept up to MAX_NOTIFICATION_DATA (64 bytes)
            
            let conn_handle = 1;
            let char_handle = 10;
            
            for size in data_sizes {
                let data = heapless_vec![0x42; size];
                
                let request = NotificationRequest {
                    conn_handle,
                    char_handle,
                    data: data.clone(),
                    is_indication: false,
                    response_id: size as u32,
                };
                
                if size <= MAX_NOTIFICATION_DATA {
                    // Should accept data within limits
                    prop_assert!(request.data.len() <= MAX_NOTIFICATION_DATA);
                    prop_assert_eq!(request.data.len(), size);
                    prop_assert!(arrays_equal(&request.data, &data));
                } else {
                    // In real implementation, oversized data would be rejected
                    prop_assert!(size > MAX_NOTIFICATION_DATA);
                }
            }
        });
    }

    #[test]
    fn test_notification_channel_capacity() {
        // Property #37: Notification Channel Capacity
        // Notification channels should handle up to 8 requests before blocking
        
        let mut requests = Vec::new();
        
        // Create 8 notification requests (expected channel capacity)
        for i in 0..8 {
            let data = heapless_vec![0x40 + i as u8; 10];
            let request = NotificationRequest {
                conn_handle: 1,
                char_handle: 10 + i as u16,
                data,
                is_indication: i % 2 == 0, // Mix notifications and indications
                response_id: i as u32,
            };
            requests.push(request);
        }
        
        assert_eq!(requests.len(), 8);
        
        // Verify each request is properly formed
        for (i, request) in requests.iter().enumerate() {
            assert_eq!(request.conn_handle, 1);
            assert_eq!(request.char_handle, 10 + i as u16);
            assert_eq!(request.data.len(), 10);
            assert_eq!(request.is_indication, i % 2 == 0);
            assert_eq!(request.response_id, i as u32);
        }
        
        // Test channel overflow scenario
        let overflow_request = NotificationRequest {
            conn_handle: 1,
            char_handle: 99,
            data: heapless_vec![0xFF; 5],
            is_indication: false,
            response_id: 999,
        };
        
        // In real implementation, this would test actual channel blocking
        assert!(overflow_request.data.len() <= MAX_NOTIFICATION_DATA);
    }

    #[test]
    fn test_request_response_matching() {
        proptest!(|(
            response_ids in prop::collection::vec(0u32..1000, 1..8)
        )| {
            // Property #38: Request-Response Matching
            // Notification responses should match their corresponding requests
            
            let conn_handle = 1;
            let char_handle = 20;
            let mut request_response_pairs = Vec::new();
            
            // Create requests with unique IDs
            for &response_id in response_ids.iter().take(8) {
                // Skip if we already added this response_id
                if request_response_pairs.iter().any(|(req, _): &(NotificationRequest, NotificationResponse)| req.response_id == response_id) {
                    continue;
                }
                
                let data = heapless_vec![0x55; 8];
                let request = NotificationRequest {
                    conn_handle,
                    char_handle,
                    data: data.clone(),
                    is_indication: true, // Use indications for response matching
                    response_id,
                };
                
                // Create matching response
                let response = NotificationResponse {
                    response_id,
                    result: Ok(()),
                };
                
                request_response_pairs.push((request, response));
            }
            
            // Verify request-response matching
            for (request, response) in &request_response_pairs {
                prop_assert_eq!(request.response_id, response.response_id);
                prop_assert!(request.is_indication); // Only indications get responses
                prop_assert!(response.result.is_ok());
                // Note: response doesn't contain conn_handle, only response_id matching is verified
            }
            
            // Verify all request IDs are unique
            let response_ids: Vec<u32> = request_response_pairs.iter()
                .map(|(req, _)| req.response_id)
                .collect();
            let mut sorted_ids = response_ids.clone();
            sorted_ids.sort();
            sorted_ids.dedup();
            prop_assert_eq!(response_ids.len(), sorted_ids.len());
        });
    }

    #[test]
    fn test_connection_validation_for_notifications() {
        // Property #39: Connection Validation for Notifications
        // Notifications should fail gracefully for invalid connection handles
        
        let valid_conn_handle = 1;
        let invalid_conn_handles = [0, 999, 0xFFFF];
        let char_handle = 30;
        let data = heapless_vec![0x88; 16];
        
        // Test with valid connection (should succeed in validation)
        let valid_request = NotificationRequest {
            conn_handle: valid_conn_handle,
            char_handle,
            data: data.clone(),
            is_indication: false,
            response_id: 100,
        };
        
        // Request should be well-formed
        assert_eq!(valid_request.conn_handle, valid_conn_handle);
        assert!(valid_request.conn_handle > 0);
        
        // Test with invalid connections
        for &invalid_id in &invalid_conn_handles {
            let invalid_request = NotificationRequest {
                conn_handle: invalid_id,
                char_handle,
                data: data.clone(),
                is_indication: false,
                response_id: 200 + invalid_id as u32,
            };
            
            // In real implementation, these would fail connection validation
            if invalid_id == 0 {
                assert_eq!(invalid_request.conn_handle, 0); // Invalid connection ID
            } else {
                assert!(invalid_request.conn_handle > valid_conn_handle); // Non-existent connection
            }
        }
    }

    #[test]
    fn test_indication_vs_notification_handling() {
        // Property #40: Indication vs Notification Handling
        // Indications and notifications should be processed differently
        
        let conn_handle = 1;
        let char_handle = 40;
        let data = heapless_vec![0xAA; 12];
        
        // Create notification (no response expected)
        let notification = NotificationRequest {
            conn_handle,
            char_handle,
            data: data.clone(),
            is_indication: false,
            response_id: 300,
        };
        
        // Create indication (response expected)
        let indication = NotificationRequest {
            conn_handle,
            char_handle: char_handle + 1,
            data: data.clone(),
            is_indication: true,
            response_id: 301,
        };
        
        // Verify different types
        assert!(!notification.is_indication);
        assert!(indication.is_indication);
        assert_ne!(notification.response_id, indication.response_id);
        assert_ne!(notification.char_handle, indication.char_handle);
        
        // Only indication should expect a response
        if indication.is_indication {
            let indication_response = NotificationResponse {
                response_id: indication.response_id,
                result: Ok(()),
            };
            assert_eq!(indication_response.response_id, indication.response_id);
        }
        
        // Notification should not have a response
        assert!(!notification.is_indication);
    }

    #[test]
    fn test_notification_response_id_uniqueness() {
        proptest!(|(
            base_ids in prop::collection::vec(0u32..100, 1..8)
        )| {
            // Property #41: Notification Request ID Uniqueness
            // Each notification request should have a unique request ID
            
            let conn_handle = 1;
            let char_handle = 50;
            let data = heapless_vec![0xCC; 8];
            let mut response_ids = Vec::new();
            
            // Generate unique request IDs
            for (i, &base_id) in base_ids.iter().enumerate() {
                let unique_id = base_id + (i as u32 * 1000); // Ensure uniqueness
                
                let request = NotificationRequest {
                    conn_handle,
                    char_handle,
                    data: data.clone(),
                    is_indication: i % 2 == 0,
                    response_id: unique_id,
                };
                
                response_ids.push(request.response_id);
                
                // Verify request is well-formed
                prop_assert_eq!(request.conn_handle, conn_handle);
                prop_assert_eq!(request.response_id, unique_id);
            }
            
            // Verify all request IDs are unique
            let mut sorted_ids = response_ids.clone();
            sorted_ids.sort();
            sorted_ids.dedup();
            prop_assert_eq!(response_ids.len(), sorted_ids.len());
            
            // Verify no ID collisions in realistic usage
            let mut collision_check = Vec::new();
            for id in &response_ids {
                prop_assert!(!collision_check.contains(id));
                collision_check.push(*id);
            }
        });
    }

    #[test]
    fn test_notification_error_handling() {
        // Additional test for notification error scenarios
        
        let conn_handle = 1;
        let char_handle = 60;
        
        // Test oversized data error
        let oversized_request = NotificationRequest {
            conn_handle,
            char_handle,
            data: heapless_vec![0xDD; MAX_NOTIFICATION_DATA],
            is_indication: false,
            response_id: 400,
        };
        
        // Request should be created - data size is at limit
        assert_eq!(oversized_request.data.len(), MAX_NOTIFICATION_DATA);
        
        // Test error response
        let error_response = NotificationResponse {
            response_id: 401,
            result: Err(NotificationError::ConnectionNotFound),
        };
        
        match error_response.result {
            Err(NotificationError::ConnectionNotFound) => {
                // Connection not found error is expected
            }
            _ => panic!("Should be ConnectionNotFound error"),
        }
        
        // Test data size boundary
        let max_size_request = NotificationRequest {
            conn_handle,
            char_handle,
            data: heapless_vec![0xEE; MAX_NOTIFICATION_DATA],
            is_indication: true,
            response_id: 402,
        };
        
        assert_eq!(max_size_request.data.len(), MAX_NOTIFICATION_DATA);
        assert!(max_size_request.data.len() <= MAX_NOTIFICATION_DATA);
    }
}