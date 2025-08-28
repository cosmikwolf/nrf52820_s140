# Phase 3B: GATT Foundation Implementation

## Phase Overview
Implement GATT Server operations to enable dynamic service creation and characteristic management. This phase provides the foundation for the BLE proxy architecture's most critical feature: allowing the host to create arbitrary GATT services at runtime.

## Success Criteria
- [ ] Host can dynamically add GATT services via commands
- [ ] Host can add characteristics with full metadata support
- [ ] MTU exchange handled properly
- [ ] Notifications and indications can be sent
- [ ] System attributes (bonding) supported
- [ ] All operations validated with hardware tests

## Required Commands

### Service Management
| Command | Function | Status | Priority |
|---------|----------|--------|----------|
| `REQ_GATTS_SERVICE_ADD (0x0080)` | `handle_service_add()` | Not implemented | Critical |

### Characteristic Management  
| Command | Function | Status | Priority |
|---------|----------|--------|----------|
| `REQ_GATTS_CHARACTERISTIC_ADD (0x0081)` | `handle_characteristic_add()` | Not implemented | Critical |

### Data Transfer
| Command | Function | Status | Priority |
|---------|----------|--------|----------|
| `REQ_GATTS_HVX (0x0083)` | `handle_hvx()` | Not implemented | High |
| `REQ_GATTS_MTU_REPLY (0x0082)` | `handle_mtu_reply()` | Not implemented | High |

### System Attributes (Bonding)
| Command | Function | Status | Priority |
|---------|----------|--------|----------|
| `REQ_GATTS_SYS_ATTR_SET (0x0085)` | `handle_sys_attr_set()` | Not implemented | Medium |
| `REQ_GATTS_SYS_ATTR_GET (0x0084)` | `handle_sys_attr_get()` | Not implemented | Low |

## Implementation Tasks

### Task 1: Dynamic Service Addition
**File**: `src/commands/gatts.rs` (to be created)

```rust
use nrf_softdevice::ble::gatt_server;
use heapless::FnvIndexMap;

pub struct GattManager {
    services: FnvIndexMap<u16, ServiceHandle, 16>, // service_id -> handle
    characteristics: FnvIndexMap<u16, CharHandle, 64>, // char_id -> handle
}

pub async fn handle_service_add(payload: &[u8]) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Parse service UUID and type (primary/secondary) from payload
    // 2. Add service to GATT server using nrf-softdevice
    // 3. Store service handle for characteristic additions
    // 4. Return service handle in response
}
```

**Payload Format** (from C analysis):
```
[UUID Type (1)] [UUID (2 or 16 bytes)] [Service Type (1)] [Service ID (2)]
```

### Task 2: Dynamic Characteristic Addition
**File**: `src/commands/gatts.rs`

```rust
pub async fn handle_characteristic_add(payload: &[u8]) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Parse characteristic definition from complex payload
    // 2. Handle characteristic metadata (properties, permissions)
    // 3. Handle CCCD (Client Characteristic Configuration Descriptor) 
    // 4. Handle SCCD (Server Characteristic Configuration Descriptor)
    // 5. Handle presentation format descriptor
    // 6. Add characteristic to specified service
    // 7. Return characteristic handle(s) in response
}
```

**Complex Payload** (from C analysis):
- Characteristic UUID and properties
- Value attribute metadata (permissions, max length, variable length flag)
- Optional user description descriptor
- Optional presentation format descriptor  
- Optional CCCD metadata
- Optional SCCD metadata

### Task 3: Notifications and Indications
**File**: `src/commands/gatts.rs`

```rust
pub async fn handle_hvx(payload: &[u8]) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Parse characteristic handle and data from payload
    // 2. Determine if notification or indication
    // 3. Send HVx (Handle Value Notification/Indication)
    // 4. Handle blocking vs non-blocking operation
    // 5. Return confirmation for indications
}
```

**HVx Types**:
- Notification: Fire-and-forget, no confirmation needed
- Indication: Requires client confirmation, blocking operation

### Task 4: MTU Exchange Handling
**File**: `src/commands/gatts.rs`

```rust
pub async fn handle_mtu_reply(payload: &[u8]) -> Result<TxPacket, CommandError> {
    // Implementation needed:
    // 1. Parse desired MTU from payload
    // 2. Reply to pending MTU exchange request
    // 3. Update effective MTU for connection
    // 4. Return final negotiated MTU
}
```

## Data Structures Needed

### Service Registry
```rust
use heapless::{FnvIndexMap, Vec};

#[derive(Clone)]
pub struct ServiceInfo {
    pub handle: u16,
    pub uuid: BleUuid,
    pub service_type: ServiceType, // Primary or Secondary
}

#[derive(Clone)]  
pub struct CharacteristicInfo {
    pub handle: u16,
    pub value_handle: u16,
    pub cccd_handle: Option<u16>,
    pub sccd_handle: Option<u16>,
    pub uuid: BleUuid,
    pub properties: CharProperties,
}

pub struct GattRegistry {
    pub services: FnvIndexMap<u16, ServiceInfo, 16>,
    pub characteristics: FnvIndexMap<u16, CharacteristicInfo, 64>,
    pub next_service_id: u16,
    pub next_char_id: u16,
}
```

### UUID Handling
```rust
#[derive(Clone, Copy)]
pub enum BleUuid {
    Uuid16(u16),
    Uuid128([u8; 16]),
    VendorSpecific { base_id: u8, offset: u16 }, // For registered UUID bases
}
```

### Characteristic Properties
```rust
bitflags::bitflags! {
    pub struct CharProperties: u8 {
        const BROADCAST = 0x01;
        const READ = 0x02;
        const WRITE_WITHOUT_RESPONSE = 0x04;
        const WRITE = 0x08;
        const NOTIFY = 0x10;
        const INDICATE = 0x20;
        const AUTH_SIGNED_WRITES = 0x40;
        const EXTENDED_PROPERTIES = 0x80;
    }
}
```

## Integration Points

### With Phase 3A (GAP Operations)
- Requires working advertising for GATT services to be discoverable
- Device name from GAP appears in Generic Access Service
- Connection establishment needed for GATT operations

### With Event System (Phase 3C)
- GATT write events must be forwarded to host
- MTU exchange events must be handled
- Connection parameter updates affect GATT performance

### With UUID Management
- Must integrate with `REQ_REGISTER_UUID_GROUP` for vendor-specific UUIDs
- Support both 16-bit standard UUIDs and 128-bit custom UUIDs

## Testing Strategy

### Unit Tests
- Test UUID parsing and conversion
- Test service/characteristic handle management
- Test payload parsing for complex characteristic definitions
- Test permission and property validation

### Hardware Tests  
- Verify services appear in GATT discovery
- Test characteristic read/write operations
- Test notification/indication delivery
- Test MTU negotiation with different client MTU values
- Test CCCD enable/disable for notifications

### Integration Tests
- Test complete service creation workflow (service + multiple characteristics)
- Test concurrent GATT operations
- Test error recovery when SoftDevice operations fail
- Test memory limits (max services/characteristics)

## SoftDevice Integration

### Required APIs
```rust
use nrf_softdevice::ble::gatt_server::{
    register_service, 
    register_characteristic,
    notify,
    indicate,
};
```

### Memory Considerations
- GATT attribute table has fixed size (1408 bytes from C analysis)  
- Each service and characteristic consumes attribute table space
- Must track remaining space and reject additions when full
- Handle allocation must be managed carefully

## Error Handling

### SoftDevice Errors
- `NRF_ERROR_NO_MEM` - Attribute table full
- `NRF_ERROR_INVALID_PARAM` - Invalid UUID or properties
- `NRF_ERROR_NOT_FOUND` - Invalid service handle for characteristic

### Protocol Errors
- Invalid service ID in characteristic addition
- Malformed UUID in payload
- Invalid permission combinations
- CCCD operations on non-notifiable characteristics

## Performance Considerations

### Memory Usage
- Pre-allocate service and characteristic registries
- Use fixed-size collections to avoid heap fragmentation
- Consider memory pools for large characteristic values

### Response Times
- Service addition is synchronous but may be slow
- Characteristic addition involves multiple SoftDevice calls
- Notifications are fire-and-forget, indications block until confirmed

## Dependencies

### Internal
- UUID registration system (`REQ_REGISTER_UUID_GROUP`)
- Connection state tracking
- Event forwarding system

### External  
- `nrf-softdevice` GATT server APIs
- `heapless` for collections
- `bitflags` for properties and permissions

## Risk Assessment

### Critical Risks
- **Attribute Table Overflow**: Must track and limit total attribute usage
- **Handle Management**: SoftDevice handle allocation must be tracked precisely  
- **UUID Base Registration**: Vendor-specific UUIDs must be registered before use

### Mitigation
- Implement attribute table usage tracking
- Create handle registry with validation
- Validate all UUIDs against registered bases before SoftDevice calls

## Next Steps to Phase 3C
- GATT operations generate events that must be forwarded to host
- Write events, MTU changes, and connection state changes need event system
- Bonding system attributes require persistent storage integration