# Test Properties for nRF52820 BLE Modem Firmware

This document outlines the key properties that need to be tested for all system components, organized by development phases.

## Phase 3A: Buffer Pool Properties ✅

### 1. Buffer Size Boundaries ✅
- **Property**: Valid data (0-249 bytes) should always allocate successfully, while oversized data (250+ bytes) should be rejected with BufferTooSmall error
- **Components**: `TxPacket::new()`, `RxBuffer::set_len()`
- **Test Strategy**: Property-based testing with random data sizes across the valid/invalid boundary
- **Implementation**: `tests/spi_tests.rs:test_buffer_size_boundaries`

### 2. Buffer Pool Exhaustion ✅
- **Property**: When all 8 TX buffers are allocated, new allocations should fail with PoolExhausted error
- **Components**: `TxPacket` allocation from `TxPool`
- **Test Strategy**: Allocate TX_POOL_SIZE buffers, then verify next allocation fails
- **Implementation**: `tests/spi_tests.rs:test_buffer_pool_exhaustion`

### 3. Buffer Data Integrity ✅
- **Property**: Data written to a buffer should be readable exactly as written, with no corruption
- **Components**: `TxPacket`, `RxBuffer`
- **Test Strategy**: Write random data patterns and verify exact readback
- **Implementation**: `tests/spi_tests.rs:test_buffer_data_integrity`

### 4. RX Buffer Length Validation ✅
- **Property**: Valid lengths (0-249) should be accepted, invalid lengths (250+) should be rejected with InvalidSize error
- **Components**: `RxBuffer::set_len()`
- **Test Strategy**: Property-based testing with length values across valid/invalid ranges
- **Implementation**: `tests/spi_tests.rs:test_rx_buffer_length_validation`

### 5. Buffer Lifecycle ✅
- **Property**: Buffers should be properly returned to the pool when dropped/freed
- **Components**: `TxPacket` Drop implementation, `atomic_pool`
- **Test Strategy**: Allocate, drop, and re-allocate to verify pool reuse
- **Implementation**: `tests/spi_tests.rs:test_buffer_lifecycle`

## Phase 3B: Protocol Packet Properties ✅

### 6. Packet Parsing Correctness ✅
- **Property**: Valid packet format [Length:2][Payload:N][RequestCode:2][CRC16:2] should parse correctly
- **Components**: `Packet::new_request()`
- **Test Strategy**: Generate valid packets with various payloads and verify correct parsing
- **Implementation**: `tests/spi_tests.rs:test_packet_serialization_roundtrip`

### 7. Packet Serialization Roundtrip ✅
- **Property**: Any packet that can be created should serialize and then parse back to identical data
- **Components**: `Packet::new_request_for_sending()`, `Packet::serialize_request()`, `Packet::new_request()`
- **Test Strategy**: Property-based testing with random request codes and payloads
- **Implementation**: `tests/spi_tests.rs:test_packet_serialization_roundtrip`

### 8. CRC Validation ✅
- **Property**: Packets with invalid CRC should be rejected with InvalidCrc error
- **Components**: `Packet::new_request()`, `validate_crc16()`
- **Test Strategy**: Generate packets with corrupted CRC values
- **Implementation**: `tests/spi_tests.rs:test_crc_validation`

### 9. Length Field Consistency ✅
- **Property**: The length field should match the actual packet size
- **Components**: `Packet::new_request()`, `Packet::serialize_request()`
- **Test Strategy**: Generate packets with mismatched length fields
- **Implementation**: `tests/spi_tests.rs:test_length_field_consistency`

### 10. Minimum Packet Size ✅
- **Property**: Packets smaller than 6 bytes should be rejected as InvalidLength
- **Components**: `Packet::new_request()`
- **Test Strategy**: Test with packets of 0-5 bytes
- **Implementation**: `tests/spi_tests.rs:test_minimum_packet_size`

### 11. Maximum Payload Size ✅
- **Property**: Payloads larger than MAX_PAYLOAD_SIZE (249 bytes) should be rejected
- **Components**: `Packet::serialize_request()`
- **Test Strategy**: Generate packets with oversized payloads
- **Implementation**: `tests/spi_tests.rs:test_maximum_payload_size`

### 12. Request Code Preservation ✅
- **Property**: Request codes should be preserved exactly through serialize/parse cycles
- **Components**: All packet operations
- **Test Strategy**: Test all valid RequestCode enum values
- **Implementation**: `tests/spi_tests.rs:test_request_code_preservation`

## Phase 3C: SPI Communication Properties ✅

### 13. Channel Capacity Limits ❌
- **Property**: TX channel (8 slots) and RX channel (1 slot) should respect their capacity limits
- **Components**: `TX_CHANNEL`, `RX_CHANNEL`
- **Test Strategy**: Fill channels to capacity and verify behavior
- **Status**: Needs implementation

### 14. Non-blocking Operations ❌
- **Property**: `try_receive_command()` should return immediately with None when no data available
- **Components**: `spi_comm::try_receive_command()`
- **Test Strategy**: Call on empty channel and verify immediate return
- **Status**: Needs implementation

### 15. Channel State Consistency ❌
- **Property**: `tx_has_space()` and `rx_has_data()` should accurately reflect channel states
- **Components**: `spi_comm::tx_has_space()`, `spi_comm::rx_has_data()`
- **Test Strategy**: Verify state functions match actual channel contents
- **Status**: Needs implementation

### 16. Error Propagation ✅
- **Property**: SPI errors should properly convert to appropriate error types (BufferError → SpiError, etc.)
- **Components**: `SpiError::From` implementations
- **Test Strategy**: Generate underlying errors and verify proper conversion
- **Implementation**: `tests/spi_tests.rs:test_error_propagation`

### 17. Data Transfer Integrity ❌
- **Property**: Data sent through SPI channels should arrive unchanged
- **Components**: `send_response()`, `receive_command()`
- **Test Strategy**: Send random data and verify exact reception
- **Status**: Needs implementation

## Phase 3C: Command Processing Properties ✅

### 18. Command Routing ✅
- **Property**: Valid request codes should be routed to appropriate handlers
- **Components**: `process_command()`, command routing logic
- **Test Strategy**: Send all valid RequestCode values and verify proper routing
- **Implementation**: `tests/command_tests.rs:test_get_info_command`, `tests/command_tests.rs:test_echo_command`

### 19. Invalid Command Handling ❌
- **Property**: Unknown request codes should return NotImplemented error
- **Components**: `process_command()`
- **Test Strategy**: Send invalid/unknown request codes
- **Status**: Needs implementation

### 20. Response Generation ✅
- **Property**: All processed commands should generate appropriate response packets
- **Components**: Command handlers, `ResponseBuilder`
- **Test Strategy**: Verify all commands produce valid responses
- **Implementation**: `tests/command_tests.rs:test_response_builder`, `tests/command_tests.rs:test_packet_creation`

### 21. Error Handling ✅
- **Property**: Command processing errors should be properly caught and converted to error responses
- **Components**: `process_command()`, error handling logic
- **Test Strategy**: Induce various error conditions and verify proper error responses
- **Implementation**: `tests/command_tests.rs:test_error_responses`

## Phase 3D: Connection Management Properties ❌

### 22. Connection Pool Management ❌
- **Property**: System should accept up to MAX_CONNECTIONS (2) and reject additional connections
- **Components**: `ConnectionManager::on_connection_established`
- **Test Strategy**: Property-based testing with connection attempts beyond limit
- **Test Type**: Traditional test with edge case verification

### 23. Connection Event Propagation ❌
- **Property**: Connection events should be properly forwarded to registered listeners
- **Components**: `ConnectionManager::emit_event`, `CONNECTION_EVENT_CHANNEL`
- **Test Strategy**: Register listeners and verify event delivery
- **Test Type**: Traditional test with async event verification

### 24. Connection State Consistency ❌
- **Property**: Connection state should remain consistent across operations
- **Components**: `ConnectionInfo` state management
- **Test Strategy**: State machine testing with random state transitions
- **Test Type**: Property-based testing with state verification

### 25. Connection Cleanup on Disconnect ❌
- **Property**: Disconnected connections should be properly cleaned up and removed
- **Components**: `ConnectionManager::on_connection_terminated`
- **Test Strategy**: Connect/disconnect cycles with resource verification
- **Test Type**: Traditional test with resource leak detection

### 26. Connection Handle Uniqueness ❌
- **Property**: Each active connection should have a unique handle
- **Components**: `ConnectionManager` handle assignment
- **Test Strategy**: Multiple simultaneous connections with handle verification
- **Test Type**: Traditional test with uniqueness verification

### 27. Connection Event Channel Capacity ❌
- **Property**: Event channel should handle up to 8 events before blocking
- **Components**: `CONNECTION_EVENT_CHANNEL`
- **Test Strategy**: Fill channel to capacity and verify overflow behavior
- **Test Type**: Traditional test with capacity verification

## Phase 3D: Dynamic GATT Properties ❌

### 28. Service Registration Limits ❌
- **Property**: System should accept up to MAX_SERVICES (4) and reject additional services
- **Components**: `ServiceManager::add_service`
- **Test Strategy**: Property-based testing with service registration beyond limits
- **Test Type**: Property-based testing with random service definitions

### 29. Characteristic Handle Assignment ❌
- **Property**: Characteristics should receive unique, sequential handles
- **Components**: `ServiceBuilder` handle assignment
- **Test Strategy**: Register multiple characteristics and verify handle uniqueness
- **Test Type**: Property-based testing with random characteristic counts

### 30. Service Building Workflow ❌
- **Property**: Service building should follow proper create→add_char→finalize workflow
- **Components**: `ServiceBuilder` state machine
- **Test Strategy**: Test invalid workflow sequences
- **Test Type**: Traditional test with workflow validation

### 31. UUID Base Registration ❌
- **Property**: UUID bases should be registered correctly and retrievable
- **Components**: `ModemState::register_uuid_base`, `ModemState::get_uuid_base`
- **Test Strategy**: Property-based testing with random UUID patterns
- **Test Type**: Property-based testing with UUID validation

### 32. UUID Base Limits ❌
- **Property**: System should accept up to 4 UUID bases and reject additional ones
- **Components**: `ModemState` UUID base storage
- **Test Strategy**: Register maximum UUID bases and verify overflow handling
- **Test Type**: Traditional test with boundary verification

### 33. Service Handle Collision Prevention ❌
- **Property**: Service handles should never collide across registrations
- **Components**: `ServiceManager` handle assignment
- **Test Strategy**: Register services with various handle ranges
- **Test Type**: Property-based testing with handle collision detection

### 34. Characteristic Property Validation ❌
- **Property**: Characteristic properties should be validated against BLE spec
- **Components**: `CharacteristicBuilder` property validation
- **Test Strategy**: Test invalid property combinations
- **Test Type**: Traditional test with property validation

### 35. GATT Table Consistency ❌
- **Property**: GATT table should remain consistent after service modifications
- **Components**: Dynamic GATT table management
- **Test Strategy**: Add/remove services and verify table integrity
- **Test Type**: Traditional test with table validation

## Phase 3D: Notification Service Properties ❌

### 36. Notification Data Size Limits ❌
- **Property**: Notifications should accept up to MAX_NOTIFICATION_DATA (64 bytes)
- **Components**: `send_notification`, `send_indication`
- **Test Strategy**: Property-based testing with random data sizes
- **Test Type**: Property-based testing with size boundary testing

### 37. Notification Channel Capacity ❌
- **Property**: Notification channels should handle up to 8 requests before blocking
- **Components**: `NOTIFICATION_CHANNEL`, `NOTIFICATION_RESPONSE_CHANNEL`
- **Test Strategy**: Fill channels to capacity and verify overflow behavior
- **Test Type**: Traditional test with capacity verification

### 38. Request-Response Matching ❌
- **Property**: Notification responses should match their corresponding requests
- **Components**: Request ID generation and matching logic
- **Test Strategy**: Send multiple concurrent notifications and verify response matching
- **Test Type**: Property-based testing with concurrent request handling

### 39. Connection Validation for Notifications ❌
- **Property**: Notifications should fail gracefully for invalid connection handles
- **Components**: `process_notification_request` connection validation
- **Test Strategy**: Send notifications to non-existent connections
- **Test Type**: Traditional test with error validation

### 40. Indication vs Notification Handling ❌
- **Property**: Indications and notifications should be processed differently
- **Components**: `NotificationRequest::is_indication` handling
- **Test Strategy**: Send both types and verify different processing paths
- **Test Type**: Traditional test with behavior verification

### 41. Notification Request ID Uniqueness ❌
- **Property**: Each notification request should have a unique request ID
- **Components**: `NOTIFICATION_REQUEST_ID` counter
- **Test Strategy**: Generate many requests and verify ID uniqueness
- **Test Type**: Property-based testing with ID collision detection

## Phase 3D: Bonding Service Properties ❌

### 42. Bonded Device Storage Limits ❌
- **Property**: System should store up to MAX_BONDED_DEVICES (2) bonded devices
- **Components**: `BondingService` device storage
- **Test Strategy**: Bond maximum devices and verify overflow handling
- **Test Type**: Traditional test with storage limit verification

### 43. Bond Data Persistence ❌
- **Property**: Bonding data should survive system resets (when flash storage added)
- **Components**: Bond data storage and retrieval
- **Test Strategy**: Store bonds, simulate reset, verify data integrity
- **Test Type**: Traditional test with persistence verification

### 44. System Attributes Size Limits ❌
- **Property**: System attributes should not exceed MAX_SYS_ATTR_SIZE (64 bytes)
- **Components**: System attribute storage
- **Test Strategy**: Property-based testing with random attribute sizes
- **Test Type**: Property-based testing with size validation

### 45. Bond Key Generation ❌
- **Property**: Bonding keys should be cryptographically secure and unique
- **Components**: Key generation and storage
- **Test Strategy**: Generate multiple keys and verify randomness/uniqueness
- **Test Type**: Property-based testing with cryptographic validation

### 46. Bond State Consistency ❌
- **Property**: Bond states should remain consistent across operations
- **Components**: `BondingService` state management
- **Test Strategy**: State machine testing with random state transitions
- **Test Type**: Traditional test with state validation

### 47. Bonding Event Propagation ❌
- **Property**: Bonding events should be properly propagated to listeners
- **Components**: Bonding event system
- **Test Strategy**: Register listeners and verify event delivery
- **Test Type**: Traditional test with event validation

## Phase 3D: Event Forwarding Properties ❌

### 48. BLE Event Classification ❌
- **Property**: BLE events should be correctly classified and routed
- **Components**: BLE event handlers
- **Test Strategy**: Generate various BLE events and verify proper classification
- **Test Type**: Traditional test with event classification

### 49. Event Handler Registration ❌
- **Property**: Event handlers should be properly registered and called
- **Components**: Event dispatcher system
- **Test Strategy**: Register handlers and verify they receive appropriate events
- **Test Type**: Traditional test with handler verification

### 50. Event Processing Order ❌
- **Property**: Events should be processed in the order they arrive
- **Components**: Event queue management
- **Test Strategy**: Send rapid sequence of events and verify processing order
- **Test Type**: Traditional test with ordering verification

### 51. Event Handler Error Isolation ❌
- **Property**: Errors in one event handler should not affect others
- **Components**: Event handler error handling
- **Test Strategy**: Inject errors into handlers and verify system stability
- **Test Type**: Traditional test with error isolation

### 52. Event Memory Management ❌
- **Property**: Event data should be properly allocated and freed
- **Components**: Event data lifecycle management
- **Test Strategy**: Process many events and verify no memory leaks
- **Test Type**: Traditional test with memory verification

### 53. Event Priority Handling ❌
- **Property**: High-priority events should be processed before low-priority ones
- **Components**: Event priority queue (if implemented)
- **Test Strategy**: Send mixed priority events and verify processing order
- **Test Type**: Traditional test with priority verification

## Phase 3D: Integration Properties ❌

### 54. Cross-Component State Consistency ❌
- **Property**: State should remain consistent across all components
- **Components**: All major components
- **Test Strategy**: Complex workflows touching multiple components
- **Test Type**: Integration test with state verification

### 55. Resource Cleanup on System Reset ❌
- **Property**: All resources should be properly cleaned up on system reset
- **Components**: All resource-managing components
- **Test Strategy**: Allocate resources, reset system, verify cleanup
- **Test Type**: Integration test with resource verification

### 56. End-to-End Command Processing ❌
- **Property**: Commands should flow correctly from SPI through all layers
- **Components**: Full command processing pipeline
- **Test Strategy**: Send commands and verify end-to-end processing
- **Test Type**: Integration test with full pipeline verification

### 57. Concurrent Operation Safety ❌
- **Property**: System should handle concurrent operations safely
- **Components**: All async components
- **Test Strategy**: Stress test with concurrent operations
- **Test Type**: Property-based testing with concurrency stress

### 58. Error Recovery Consistency ❌
- **Property**: Error recovery should work consistently across components
- **Components**: All error-handling components
- **Test Strategy**: Inject various errors and verify recovery
- **Test Type**: Integration test with error injection

### 59. Memory Usage Under Load ❌
- **Property**: Memory usage should remain within bounds under heavy load
- **Components**: All memory-using components
- **Test Strategy**: Stress test with maximum allocations
- **Test Type**: Integration test with memory monitoring

### 60. Task Coordination ❌
- **Property**: Embassy tasks should coordinate properly without deadlocks
- **Components**: All async tasks
- **Test Strategy**: Complex async workflows with timing variations
- **Test Type**: Integration test with async coordination

### 61. BLE Stack Integration ❌
- **Property**: Integration with nrf_softdevice should work correctly
- **Components**: BLE stack interface
- **Test Strategy**: Full BLE workflows with actual stack calls
- **Test Type**: Integration test with BLE stack verification

### 62. Performance Under Load ❌
- **Property**: System should maintain performance under maximum load
- **Components**: All components under stress
- **Test Strategy**: Maximum throughput testing
- **Test Type**: Performance test with load verification

### 63. System Stability ❌
- **Property**: System should remain stable during extended operation
- **Components**: Entire system
- **Test Strategy**: Long-running stress test
- **Test Type**: Stability test with extended runtime

## Phase 4: Test Implementation Plan

### Test Status Summary
- **Phase 3A-3C**: 16/21 properties tested (76% coverage)
- **Phase 3D**: 0/42 properties tested (0% coverage)
- **Overall**: 16/63 properties tested (25% coverage)

### Priority Implementation Order
1. **Connection Tests**: Properties 22-27 (connection_tests.rs)
2. **GATT Tests**: Properties 28-35 (gatt_tests.rs)
3. **Notification Tests**: Properties 36-41 (notification_tests.rs)
4. **Bonding Tests**: Properties 42-47 (bonding_tests.rs)
5. **Event Tests**: Properties 48-53 (event_tests.rs)
6. **Integration Tests**: Properties 54-63 (end_to_end_tests.rs)

### Remaining Phase 3C Properties
- Properties 13, 14, 15, 17, 19 need implementation in existing test files

### New Test Files Required
- `tests/connection_tests.rs` - Connection management testing
- `tests/gatt_tests.rs` - Dynamic GATT service testing
- `tests/notification_tests.rs` - Notification service testing
- `tests/bonding_tests.rs` - Bonding service testing
- `tests/event_tests.rs` - Event forwarding testing
- `tests/end_to_end_tests.rs` - Integration testing

## Implementation Notes

### Property-Based Testing Strategy
- Use `proptest!` macros for properties involving ranges of inputs (marked as "Property-based testing")
- Generate random data patterns, sizes, and request codes
- Test boundary conditions explicitly
- Verify both positive and negative cases

### Traditional Testing Strategy
- Use standard `#[test]` functions for workflow and state validation (marked as "Traditional test")
- Focus on complex async coordination and integration scenarios
- Verify proper error handling and resource cleanup
- Test specific behavior patterns and state machines

### Test Organization
- Group related properties into test modules by component
- Use descriptive test names that reflect the property being tested
- Include both unit tests and integration tests
- Ensure tests are deterministic despite using randomness
- Follow existing patterns from `tests/spi_tests.rs`

### Coverage Considerations
- Test all enum variants (RequestCode, ResponseCode, error types)
- Test edge cases (empty payloads, maximum sizes, boundary values)
- Test error paths as thoroughly as success paths
- Verify resource cleanup and proper state management
- Focus on memory constraints and Embassy async patterns

### Testing Infrastructure
- All tests use `#[defmt_test::tests]` for embedded testing
- Property-based tests use `proptest!` macros
- Heap initialization required for dynamic allocation tests
- Use `arrays_equal` helper for byte array comparisons
- Follow CLAUDE.md guidelines: no unnecessary prints, proper asserts