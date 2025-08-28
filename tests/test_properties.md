# SPI Communication System Test Properties

This document outlines the key properties that need to be tested for the SPI communication system and related components.

## Buffer Pool Properties

### 1. Buffer Size Boundaries
- **Property**: Valid data (0-249 bytes) should always allocate successfully, while oversized data (250+ bytes) should be rejected with BufferTooSmall error
- **Components**: `TxPacket::new()`, `RxBuffer::set_len()`
- **Test Strategy**: Property-based testing with random data sizes across the valid/invalid boundary

### 2. Buffer Pool Exhaustion
- **Property**: When all 8 TX buffers are allocated, new allocations should fail with PoolExhausted error
- **Components**: `TxPacket` allocation from `TxPool`
- **Test Strategy**: Allocate TX_POOL_SIZE buffers, then verify next allocation fails

### 3. Buffer Data Integrity
- **Property**: Data written to a buffer should be readable exactly as written, with no corruption
- **Components**: `TxPacket`, `RxBuffer`
- **Test Strategy**: Write random data patterns and verify exact readback

### 4. RX Buffer Length Validation
- **Property**: Valid lengths (0-249) should be accepted, invalid lengths (250+) should be rejected with InvalidSize error
- **Components**: `RxBuffer::set_len()`
- **Test Strategy**: Property-based testing with length values across valid/invalid ranges

### 5. Buffer Lifecycle
- **Property**: Buffers should be properly returned to the pool when dropped/freed
- **Components**: `TxPacket` Drop implementation, `atomic_pool`
- **Test Strategy**: Allocate, drop, and re-allocate to verify pool reuse

## Protocol Packet Properties

### 6. Packet Parsing Correctness
- **Property**: Valid packet format [Length:2][Payload:N][RequestCode:2][CRC16:2] should parse correctly
- **Components**: `Packet::new_request()`
- **Test Strategy**: Generate valid packets with various payloads and verify correct parsing

### 7. Packet Serialization Roundtrip
- **Property**: Any packet that can be created should serialize and then parse back to identical data
- **Components**: `Packet::new_request_for_sending()`, `Packet::serialize_request()`, `Packet::new_request()`
- **Test Strategy**: Property-based testing with random request codes and payloads

### 8. CRC Validation
- **Property**: Packets with invalid CRC should be rejected with InvalidCrc error
- **Components**: `Packet::new_request()`, `validate_crc16()`
- **Test Strategy**: Generate packets with corrupted CRC values

### 9. Length Field Consistency
- **Property**: The length field should match the actual packet size
- **Components**: `Packet::new_request()`, `Packet::serialize_request()`
- **Test Strategy**: Generate packets with mismatched length fields

### 10. Minimum Packet Size
- **Property**: Packets smaller than 6 bytes should be rejected as InvalidLength
- **Components**: `Packet::new_request()`
- **Test Strategy**: Test with packets of 0-5 bytes

### 11. Maximum Payload Size
- **Property**: Payloads larger than MAX_PAYLOAD_SIZE (249 bytes) should be rejected
- **Components**: `Packet::serialize_request()`
- **Test Strategy**: Generate packets with oversized payloads

### 12. Request Code Preservation
- **Property**: Request codes should be preserved exactly through serialize/parse cycles
- **Components**: All packet operations
- **Test Strategy**: Test all valid RequestCode enum values

## SPI Communication Properties

### 13. Channel Capacity Limits
- **Property**: TX channel (8 slots) and RX channel (1 slot) should respect their capacity limits
- **Components**: `TX_CHANNEL`, `RX_CHANNEL`
- **Test Strategy**: Fill channels to capacity and verify behavior

### 14. Non-blocking Operations
- **Property**: `try_receive_command()` should return immediately with None when no data available
- **Components**: `spi_comm::try_receive_command()`
- **Test Strategy**: Call on empty channel and verify immediate return

### 15. Channel State Consistency
- **Property**: `tx_has_space()` and `rx_has_data()` should accurately reflect channel states
- **Components**: `spi_comm::tx_has_space()`, `spi_comm::rx_has_data()`
- **Test Strategy**: Verify state functions match actual channel contents

### 16. Error Propagation
- **Property**: SPI errors should properly convert to appropriate error types (BufferError → SpiError, etc.)
- **Components**: `SpiError::From` implementations
- **Test Strategy**: Generate underlying errors and verify proper conversion

### 17. Data Transfer Integrity
- **Property**: Data sent through SPI channels should arrive unchanged
- **Components**: `send_response()`, `receive_command()`
- **Test Strategy**: Send random data and verify exact reception

## Command Processing Properties

### 18. Command Routing
- **Property**: Valid request codes should be routed to appropriate handlers
- **Components**: `process_command()`, command routing logic
- **Test Strategy**: Send all valid RequestCode values and verify proper routing

### 19. Invalid Command Handling
- **Property**: Unknown request codes should return NotImplemented error
- **Components**: `process_command()`
- **Test Strategy**: Send invalid/unknown request codes

### 20. Response Generation
- **Property**: All processed commands should generate appropriate response packets
- **Components**: Command handlers, `ResponseBuilder`
- **Test Strategy**: Verify all commands produce valid responses

### 21. Error Handling
- **Property**: Command processing errors should be properly caught and converted to error responses
- **Components**: `process_command()`, error handling logic
- **Test Strategy**: Induce various error conditions and verify proper error responses

## Implementation Notes

### Property-Based Testing Strategy
- Use `proptest!` macros for properties involving ranges of inputs
- Generate random data patterns, sizes, and request codes
- Test boundary conditions explicitly
- Verify both positive and negative cases

### Test Organization
- Group related properties into test modules
- Use descriptive test names that reflect the property being tested
- Include both unit tests and integration tests
- Ensure tests are deterministic despite using randomness

### Coverage Considerations
- Test all enum variants (RequestCode, ResponseCode, error types)
- Test edge cases (empty payloads, maximum sizes, boundary values)
- Test error paths as thoroughly as success paths
- Verify resource cleanup and proper state management