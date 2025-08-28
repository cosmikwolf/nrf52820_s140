# Phase A: SPI Communication Layer

## Overview
Establish dual SPI communication between the nRF52820 BLE modem and host processor, implementing the foundational transport layer for all command/response and event forwarding.

**CURRENT STATUS: 80% COMPLETE** - Core implementation exists, needs CRC16 and testing

## Goals
- ✅ Configure dual SPI interfaces (TX: SPIM0 master, RX: SPIS1 slave) **[DONE]**
- ⚠️ Implement message framing/deframing with CRC16 **[PARTIAL - No CRC]**
- ✅ Create async SPI tasks with proper interrupt priorities **[DONE]**
- ✅ Establish buffer pool management for zero-copy transfers **[DONE]**
- 🔴 Verify bidirectional communication with loopback testing **[TODO]**

## Technical Requirements

### SPI Configuration
```
TX SPI (SPIM0 - Master for Device→Host):
- Pins: SS=P0.01, SCK=P0.00, MOSI=P0.04
- Speed: 8MHz
- Mode: CPOL=High, CPHA=Leading
- DMA enabled
- Interrupt Priority: 2

RX SPI (SPIS1 - Slave for Host→Device):
- Pins: SS=P0.07, SCK=P0.06, MOSI=P0.05  
- Mode: CPOL=High, CPHA=Leading
- DMA enabled
- Interrupt Priority: 7
```

### Message Format
```
[Length:2][Opcode:2][Payload:N][CRC16:2]
- Length: Total message size (big-endian)
- Opcode: Command/Response code (big-endian)
- Payload: Variable length data (max 243 bytes)
- CRC16: CCITT CRC16 over entire message
```

## Implementation Tasks

### 1. Core SPI Module Setup
**File: `src/spi_comm.rs`** ✅ **COMPLETE**
- [x] Configure SPIM0 for TX using embassy-nrf
- [x] Configure SPIS1 for RX using embassy-nrf
- [x] Set up pin assignments per specification
- [x] Configure DMA buffers (249 bytes each)
- [x] Set interrupt priorities via embassy config

### 2. Buffer Pool Implementation
**File: `src/buffer_pool.rs`** ✅ **COMPLETE**
- [x] Create TX buffer pool (8 buffers × 249 bytes)
- [x] Implement buffer allocation/release mechanism
- [x] Add RX command buffer with double buffering
- [x] Use `atomic-pool` for static allocation (instead of heapless::pool)
- [x] Implement zero-copy buffer passing

### 3. Message Protocol Layer
**File: `src/protocol.rs`** ⚠️ **PARTIAL - Needs CRC16**
- [x] Implement basic message structure
- [ ] Add CRC16-CCITT calculation and validation
- [x] Create packet serialization
- [x] Create packet parsing
- [ ] Handle partial message reception with proper framing

### 4. Async SPI Tasks
**File: `src/spi_comm.rs`** ✅ **COMPLETE**
```rust
#[embassy_executor::task]
async fn spi_rx_task(
    spi: Spis<'static, SPIS1>,
    cmd_queue: &'static CmdQueue,
) {
    // Continuously receive commands from host
    // Parse frames and queue for processing
}

#[embassy_executor::task]
async fn spi_tx_task(
    spi: Spim<'static, SPIM0>,
    tx_queue: &'static TxQueue,
) {
    // Continuously send responses/events to host
    // Pull from TX queue and transmit
}
```

### 5. Integration Points ⚠️ **PARTIAL**
- [ ] Add SPI tasks to main executor **[TODO - currently commented out]**
- [x] Create command and response channels using embassy-sync
- [x] Wire up buffer pools to SPI tasks
- [x] Add initialization functions

## Testing Strategy

### 1. Hardware Tests with probe-rs
**File: `tests/spi_loopback.rs`**
```rust
#[defmt_test::tests]
mod tests {
    use defmt_test::*;
    
    #[test]
    fn spi_loopback_basic() {
        // Physical wire: TX MOSI (P0.04) → RX MOSI (P0.05)
        // Send test patterns and verify reception
    }
    
    #[test]
    fn spi_crc_validation() {
        // Test CRC validation (correct and corrupted)
    }
    
    #[test] 
    fn spi_latency_test() {
        // Measure round-trip latency < 100μs
    }
}
```
**Run with:** `cargo test --test spi_loopback`

### 2. Integration Tests
**File: `tests/spi_commands.rs`**
```rust
#[defmt_test::tests]
mod tests {
    #[test]
    fn command_echo_test() {
        // Test full command/response flow
    }
    
    #[test]
    fn command_error_handling() {
        // Test error responses and recovery
    }
}
```

### 3. Unit Tests
**File: `tests/protocol_unit.rs`**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_crc16_calculation() {
        // Pure logic test - no hardware needed
    }
    
    #[test]
    fn test_packet_serialization() {
        // Test protocol parsing/serialization
    }
}
```

### 4. Performance Benchmarks
- Target: < 100μs command response latency
- Throughput: > 1Mbps sustained
- Buffer utilization: < 50% under normal load

## Dependencies
```toml
# Cargo.toml additions - UPDATED
[dependencies]
embassy-nrf = { features = ["nrf52820"] } # ✅ Present
atomic-pool = "1.0"  # ✅ Present (used instead of heapless::pool)
crc = "3.0"  # ✅ ADDED - For CRC16-CCITT
defmt-test = "0.3"  # ✅ ADDED - For hardware testing with probe-rs
```

### Test Configuration
Tests run on hardware via probe-rs:
```bash
# Run hardware SPI tests
cargo test --test spi_loopback

# Run integration tests  
cargo test --test spi_commands

# Run unit tests
cargo test --test protocol_unit
```

## Success Criteria
1. ✅ Bidirectional SPI communication established
2. ✅ Messages properly framed and validated
3. ✅ Loopback test passes with 100% reliability
4. ✅ Performance meets latency requirements
5. ✅ No memory leaks after 1 hour continuous operation

## Risk Mitigation
- **Risk**: SPI slave timing issues
  - **Mitigation**: Use DMA for reliable slave reception
- **Risk**: Buffer pool exhaustion
  - **Mitigation**: Implement back-pressure mechanism
- **Risk**: Interrupt priority conflicts with SoftDevice
  - **Mitigation**: Use P2/P7 priorities, avoid P0/P1/P4

## Next Phase
Once SPI communication is verified, Phase B will build the command processing infrastructure on top of this transport layer.