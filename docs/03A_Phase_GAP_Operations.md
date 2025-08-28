# Phase 3A: GAP Operations Implementation

## Phase Overview
Implement core GAP (Generic Access Profile) operations to match the C firmware command set. This phase focuses on device identity, advertising, and basic connection management.

## Success Criteria
- [ ] All GAP commands return proper responses instead of placeholder ACKs
- [ ] Device address can be read and configured
- [ ] Device name can be read and configured  
- [ ] Advertising can be started, stopped, and configured with custom data
- [ ] Connection parameters can be managed
- [ ] All operations tested with hardware tests

## Required Commands

### Device Identity Management
| Command | Function | Status | Priority |
|---------|----------|--------|----------|
| `REQ_GAP_GET_ADDR (0x0011)` | `handle_get_addr()` | Placeholder | High |
| `REQ_GAP_SET_ADDR (0x0012)` | `handle_set_addr()` | Placeholder | High |
| `REQ_GAP_GET_NAME (0x0023)` | `handle_get_name()` | Placeholder | High |
| `REQ_GAP_SET_NAME (0x0024)` | `handle_set_name()` | Placeholder | High |

### Advertising Control
| Command | Function | Status | Priority |
|---------|----------|--------|----------|
| `REQ_GAP_ADV_START (0x0020)` | `handle_adv_start()` | Placeholder | Critical |
| `REQ_GAP_ADV_STOP (0x0021)` | `handle_adv_stop()` | Placeholder | Critical |
| `REQ_GAP_ADV_SET_CONFIGURE (0x0022)` | `handle_adv_configure()` | Placeholder | Critical |

### Connection Parameters
| Command | Function | Status | Priority |
|---------|----------|--------|----------|
| `REQ_GAP_CONN_PARAMS_GET (0x0025)` | `handle_conn_params_get()` | Placeholder | Medium |
| `REQ_GAP_CONN_PARAMS_SET (0x0026)` | `handle_conn_params_set()` | Placeholder | Medium |
| `REQ_GAP_CONN_PARAM_UPDATE (0x0027)` | `handle_conn_param_update()` | Placeholder | Medium |

### Advanced Features (Lower Priority)
| Command | Function | Status | Priority |
|---------|----------|--------|----------|
| `REQ_GAP_DATA_LENGTH_UPDATE (0x0028)` | `handle_data_length_update()` | Placeholder | Low |
| `REQ_GAP_PHY_UPDATE (0x0029)` | `handle_phy_update()` | Placeholder | Low |
| `REQ_GAP_SET_TX_POWER (0x002D)` | `handle_set_tx_power()` | Placeholder | Low |
| `REQ_GAP_START_RSSI_REPORTING (0x002E)` | `handle_start_rssi_reporting()` | Placeholder | Low |
| `REQ_GAP_STOP_RSSI_REPORTING (0x002F)` | `handle_stop_rssi_reporting()` | Placeholder | Low |

## Implementation Tasks

### Task 1: Device Address Management
**File**: `src/commands/gap.rs:16-24`

```rust
pub async fn handle_get_addr(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Get BLE address from SoftDevice
    // 2. Format response with 6-byte address
    // 3. Return formatted response packet
}

pub async fn handle_set_addr(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Parse 6-byte address from payload
    // 2. Set BLE address in SoftDevice
    // 3. Return ACK or error response
}
```

**SoftDevice APIs needed**:
- `nrf_softdevice::ble::gap::address_get()`
- `nrf_softdevice::ble::gap::address_set()`

### Task 2: Device Name Configuration
**File**: `src/commands/gap.rs:41-48`

```rust
pub async fn handle_get_name(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Get device name from GAP
    // 2. Format response with name string
    // 3. Handle variable-length name
}

pub async fn handle_set_name(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Parse name string from payload
    // 2. Set device name in GAP
    // 3. Return ACK or error response
}
```

### Task 3: Advertising Control (Critical Path)
**File**: `src/commands/gap.rs:26-39`

```rust
pub async fn handle_adv_start(_payload: &[u8], _sd: &Softdevice) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Parse advertising parameters from payload
    // 2. Start advertising with configured parameters
    // 3. Handle advertising handle management
    // 4. Return ACK with advertising handle or error
}

pub async fn handle_adv_configure(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Parse advertising data and scan response data
    // 2. Configure advertising parameters (interval, type, etc.)
    // 3. Store configuration for advertising start
    // 4. Return ACK or validation error
}
```

**SoftDevice APIs needed**:
- `nrf_softdevice::ble::gap::advertise()`
- `nrf_softdevice::ble::gap::ConnectableAdvertisement`
- Advertising data configuration structures

## Data Structures Needed

### Address Structure
```rust
#[derive(Clone, Copy)]
pub struct BleAddress {
    pub addr: [u8; 6],
    pub addr_type: u8, // 0 = Public, 1 = Random
}
```

### Advertising Configuration
```rust
pub struct AdvConfig {
    pub data: heapless::Vec<u8, 31>,        // Advertising data (max 31 bytes)
    pub scan_response: heapless::Vec<u8, 31>, // Scan response data (max 31 bytes)
    pub interval_min: u16,                   // Advertising interval min (0.625ms units)
    pub interval_max: u16,                   // Advertising interval max (0.625ms units)
    pub timeout: u16,                        // Advertising timeout (seconds, 0 = no timeout)
    pub adv_type: u8,                        // Advertising type
}
```

## Testing Strategy

### Unit Tests
- Test address parsing and formatting
- Test name string handling (including UTF-8 validation)
- Test advertising data validation and limits
- Test parameter range validation

### Hardware Tests
- Verify actual BLE address reading/setting
- Verify device name appears correctly in scans
- Verify advertising data is transmitted correctly
- Test advertising start/stop state transitions

### Integration Tests  
- Test command-response protocol for each command
- Test error handling for invalid parameters
- Test concurrent operation handling

## Dependencies

### Internal Dependencies
- `buffer_pool::TxPacket` - Response packet allocation
- `commands::ResponseBuilder` - Response formatting
- SoftDevice must be initialized and enabled

### External Dependencies
- `nrf-softdevice` GAP APIs
- `heapless` for fixed-size collections
- Embassy async runtime

## Risk Assessment

### High Risk
- **Advertising Handle Management**: SoftDevice uses handles that must be tracked
- **Address Type Handling**: Random vs Public address complexity
- **Concurrent Operations**: Multiple GAP operations in flight

### Medium Risk
- **Parameter Validation**: Invalid ranges cause SoftDevice errors
- **Memory Management**: Advertising data must be kept alive during advertising
- **UTF-8 Names**: Device name encoding validation

### Mitigation Strategies
- Implement comprehensive parameter validation before SoftDevice calls
- Use state machines to track advertising and connection states
- Pre-allocate advertising data buffers to ensure lifetime guarantees

## Next Phase Dependencies
- Phase 3B (GATT Foundation) requires connection establishment working
- Device name and address must be functional for proper service discovery
- Advertising must work for any BLE connections to be possible