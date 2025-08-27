# BLE Modem Rust Implementation Plan

## Current State Analysis
We have a working embassy-nrf + nrf-softdevice foundation with:
- ✅ Embassy async runtime configured
- ✅ SoftDevice S140 initialized with correct parameters (MTU=247, 1 connection, etc.)
- ✅ Basic BLE peripheral advertising working
- ✅ GATT server framework in place
- ✅ Correct memory layout for nRF52820 + S140

## Architecture Decision
Given our embassy-nrf foundation, we'll implement the BLE modem using **embassy's async SPI drivers** for better integration with the existing async runtime, rather than raw register access.

## Implementation Phases

### Phase 1: Dual SPI Communication Layer (Week 1)

#### 1.1 SPI Hardware Setup
```rust
// src/spi_comm.rs
use embassy_nrf::spim::{self, Spim};
use embassy_nrf::spis::{self, Spis};

// TX SPI (SPIM0 - Master for Device→Host)
// Pins: SS=P0.01, SCK=P0.00, MOSI=P0.04
// Config: 8MHz, CPOL=High, CPHA=Leading

// RX SPI (SPIS1 - Slave for Host→Device)  
// Pins: SS=P0.07, SCK=P0.06, MOSI=P0.05
// Config: CPOL=High, CPHA=Leading
```

**Tasks:**
1. Create `src/spi_comm.rs` module with dual SPI configuration
2. Implement TX buffer pool (8 buffers × 249 bytes) using `heapless::pool`
3. Implement RX command buffer with interrupt-driven reception
4. Add DMA-based async SPI transfers using embassy's SPIM/SPIS
5. Set interrupt priorities (TX=2, RX=7) via embassy config

#### 1.2 Protocol Handler
```rust
// src/protocol.rs
#[repr(u16)]
enum RequestCode {
    GetInfo = 0x0001,
    Shutdown = 0x0002,
    RegisterUuid = 0x0010,
    // ... all command codes
}

#[repr(u16)]
enum ResponseCode {
    Ack = 0xAC50,
    BleEvent = 0x8001,
    SocEvent = 0x8002,
}
```

**Tasks:**
1. Define all request/response enums with proper `#[repr(u16)]`
2. Implement message framing/deframing
3. Create command dispatcher using match statement
4. Add big-endian serialization for protocol compatibility

### Phase 2: Command Processing Infrastructure (Week 1-2)

#### 2.1 Command Router
```rust
// src/commands/mod.rs
pub async fn handle_command(
    cmd: RequestCode,
    payload: &[u8],
    sd: &Softdevice,
    state: &mut ModemState,
) -> Result<Response, Error>
```

**Module Structure:**
```
src/commands/
├── mod.rs          // Router and common types
├── system.rs       // System commands (version, reboot)
├── gap.rs          // GAP operations
├── gatts.rs        // GATT Server operations
└── uuid.rs         // UUID management
```

#### 2.2 State Management
```rust
// src/state.rs
pub struct ModemState {
    uuid_bases: heapless::Vec<Uuid128, 4>,
    advertising_handle: Option<AdvHandle>,
    connection: Option<Connection>,
    services: heapless::Vec<ServiceHandle, 16>,
    // ... other state
}
```

**Tasks:**
1. Implement stateful command handler
2. Add UUID base registration storage
3. Track advertising and connection state
4. Manage dynamic GATT structure

### Phase 3: Dynamic GATT Service Creation (Week 2)

#### 3.1 Service Builder
Since nrf-softdevice uses compile-time GATT definition via macros, we need a different approach:

**Option A: Raw SoftDevice Calls**
```rust
// src/gatt_builder.rs
use nrf_softdevice::raw;

pub async fn add_service(
    uuid: &Uuid,
    service_type: ServiceType,
) -> Result<ServiceHandle, Error> {
    let mut handle = 0u16;
    unsafe {
        raw::sd_ble_gatts_service_add(
            service_type as u8,
            uuid.as_raw_ptr(),
            &mut handle,
        )
    }
    // ...
}
```

**Option B: Pre-allocated Generic Services**
Create a pool of generic services/characteristics that can be configured at runtime.

**Recommended: Option A** - Direct SoftDevice calls for true dynamic creation

**Tasks:**
1. Implement service creation via `sd_ble_gatts_service_add`
2. Implement characteristic addition via `sd_ble_gatts_characteristic_add`
3. Handle metadata (CCCD, SCCD, permissions)
4. Create attribute value storage management

### Phase 4: Event Forwarding System (Week 2-3)

#### 4.1 Event Capture
```rust
// src/events.rs
#[embassy_executor::task]
pub async fn event_forwarder(
    sd: &'static Softdevice,
    tx_queue: &'static TxQueue,
) {
    loop {
        // Use nrf-softdevice's event stream
        match sd.events().next().await {
            SoftdeviceEvent::Ble(evt) => {
                let packet = serialize_ble_event(evt);
                tx_queue.enqueue(packet).await;
            }
            // ...
        }
    }
}
```

**Tasks:**
1. Hook into nrf-softdevice event stream
2. Serialize events with proper format
3. Append event-specific data (advertising data for connections)
4. Queue for SPI transmission

#### 4.2 Special Event Handling
- `BLE_GAP_EVT_CONNECTED`: Append both advertising and scan response data
- `BLE_GATTS_EVT_WRITE`: Forward write data to host
- `BLE_GAP_EVT_RSSI_CHANGED`: Include RSSI value

### Phase 5: GAP Operations (Week 3)

#### 5.1 Core GAP Commands
Leverage nrf-softdevice's existing abstractions where possible:

```rust
// src/commands/gap.rs
pub async fn start_advertising(
    sd: &Softdevice,
    config: &AdvConfig,
) -> Result<(), Error> {
    // Use nrf_softdevice::ble::peripheral::advertise_connectable
    // But with host-provided advertising data
}
```

**Tasks:**
1. Implement advertising start/stop/configure
2. Add connection parameter management
3. Implement TX power control
4. Add RSSI reporting start/stop
5. Handle PHY updates (1M/2M)
6. Implement data length extension

### Phase 6: Integration & Testing (Week 3-4)

#### 6.1 Main Task Orchestration
```rust
// Modified main.rs
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // ... existing init ...
    
    // Spawn new tasks
    spawner.spawn(spi_rx_task(/* ... */)).unwrap();
    spawner.spawn(spi_tx_task(/* ... */)).unwrap();
    spawner.spawn(command_processor(/* ... */)).unwrap();
    spawner.spawn(event_forwarder(/* ... */)).unwrap();
    
    // Keep existing softdevice_task
}
```

#### 6.2 Testing Strategy
1. **SPI Loopback Test**: Verify protocol with test harness
2. **Command Response Test**: Test each command individually
3. **Event Forwarding Test**: Verify all events reach host
4. **Integration Test**: Connect with existing host processor
5. **Stress Test**: Handle rapid command sequences

## Key Technical Challenges & Solutions

### Challenge 1: Dynamic GATT with Static Macros
**Solution**: Use raw SoftDevice API calls bypassing nrf-softdevice's macro system for dynamic service creation.

### Challenge 2: Async SPI Slave
**Solution**: Embassy's SPIS driver supports async operation with DMA. Use `embassy_nrf::spis::Spis` with interrupt-driven command reception.

### Challenge 3: Buffer Management
**Solution**: Use `heapless::pool` for static allocation of TX buffers, avoiding heap fragmentation.

### Challenge 4: Event Priority
**Solution**: Configure interrupt priorities via embassy's config to match original (TX=2, RX=7, SD=4).

## Dependencies to Add
```toml
# Cargo.toml additions
embassy-nrf = { features = ["spim0", "spis1"] }  # Add SPI features
heapless = { features = ["pool"] }                # For buffer pools
postcard = "1.0"                                  # For serialization
```

## File Structure
```
src/
├── main.rs              # Modified with new tasks
├── spi_comm.rs          # Dual SPI communication
├── protocol.rs          # Message protocol definitions
├── state.rs             # Modem state management
├── events.rs            # Event forwarding system
├── commands/
│   ├── mod.rs           # Command router
│   ├── system.rs        # System commands
│   ├── gap.rs           # GAP operations
│   ├── gatts.rs         # GATTS operations
│   └── uuid.rs          # UUID management
├── gatt_builder.rs      # Dynamic GATT creation
├── buffer_pool.rs       # TX/RX buffer management
└── services.rs          # Keep for future fixed services

```

## Success Metrics
1. **Protocol Compatibility**: Existing host can communicate without changes
2. **Command Coverage**: All essential commands implemented
3. **Event Completeness**: All BLE events forwarded correctly
4. **Performance**: < 1ms command response time
5. **Stability**: 24+ hour operation without memory leaks

## Migration Path
1. **Week 1**: SPI communication + basic commands
2. **Week 2**: Dynamic GATT + event forwarding
3. **Week 3**: Complete GAP operations
4. **Week 4**: Testing & debugging with real host

## Future Enhancements (Post-MVP)
1. Add fixed GATT services alongside dynamic ones
2. Implement bonding/security features
3. Add Central role support (currently disabled)
4. Optimize protocol for reduced latency
5. Add command batching for efficiency