# Phase 3C: BLE Event System Implementation

## Phase Overview
Implement comprehensive BLE event forwarding to match the C firmware's event-driven architecture. All SoftDevice events must be captured, packaged, and transmitted to the host processor over the TX SPI channel.

## Success Criteria
- [ ] All BLE events forwarded to host with complete payloads
- [ ] Connection events include advertising and scan response data
- [ ] Event packaging matches C firmware protocol format
- [ ] Event transmission over TX SPI with proper response codes
- [ ] Event buffering handles burst scenarios
- [ ] System events (SOC) also forwarded when relevant
- [ ] **CRITICAL CONSTRAINT**: Implementation must fit in remaining 1-2KB RAM
- [ ] **Performance**: Event latency < 5ms to prevent SoftDevice overflow

## Event Categories

### Connection Management Events
| Event | Response Code | Priority | Data Included |
|-------|---------------|----------|---------------|
| `BLE_GAP_EVT_CONNECTED` | `RSP_EVENT_BLE (0x8001)` | Critical | Connection params + advertising data + scan response |
| `BLE_GAP_EVT_DISCONNECTED` | `RSP_EVENT_BLE (0x8001)` | Critical | Disconnect reason |
| `BLE_GAP_EVT_CONN_PARAM_UPDATE` | `RSP_EVENT_BLE (0x8001)` | High | New connection parameters |
| `BLE_GAP_EVT_CONN_PARAM_UPDATE_REQUEST` | `RSP_EVENT_BLE (0x8001)` | High | Requested parameters |

### GATT Server Events
| Event | Response Code | Priority | Data Included |
|-------|---------------|----------|---------------|
| `BLE_GATTS_EVT_WRITE` | `RSP_EVENT_BLE (0x8001)` | Critical | Handle, offset, data |
| `BLE_GATTS_EVT_MTU_EXCHANGE` | `RSP_EVENT_BLE (0x8001)` | High | Requested MTU |
| `BLE_GATTS_EVT_HVN_TX_COMPLETE` | `RSP_EVENT_BLE (0x8001)` | Medium | Notification complete |
| `BLE_GATTS_EVT_HVC` | `RSP_EVENT_BLE (0x8001)` | Medium | Indication confirmed |

### PHY and Data Length Events
| Event | Response Code | Priority | Data Included |
|-------|---------------|----------|---------------|
| `BLE_GAP_EVT_PHY_UPDATE` | `RSP_EVENT_BLE (0x8001)` | Medium | New PHY settings |
| `BLE_GAP_EVT_DATA_LENGTH_UPDATE` | `RSP_EVENT_BLE (0x8001)` | Medium | New data length params |

### RSSI and Signal Events  
| Event | Response Code | Priority | Data Included |
|-------|---------------|----------|---------------|
| `BLE_GAP_EVT_RSSI_CHANGED` | `RSP_EVENT_BLE (0x8001)` | Low | New RSSI value |

### System Events
| Event | Response Code | Priority | Data Included |
|-------|---------------|----------|---------------|
| SOC Events | `RSP_EVENT_SOC (0x8002)` | Variable | Event-specific data |

## Implementation Tasks

### Task 1: Event Handler Registration
**File**: `src/event_handler.rs` (to be created)

```rust
use nrf_softdevice::{Softdevice, ble::{Connection, gatt_server}};
use embassy_sync::channel::Channel;
use crate::buffer_pool::{TxPacket, TX_POOL};

pub struct EventHandler {
    tx_channel: Channel<NoopRawMutex, TxPacket, 8>,
}

impl EventHandler {
    pub fn new(tx_channel: Channel<NoopRawMutex, TxPacket, 8>) -> Self {
        Self { tx_channel }
    }
    
    pub async fn run(self, sd: &Softdevice) -> ! {
        loop {
            // Main event handling loop
            match sd.run().await {
                Event::Ble(ble_evt) => self.handle_ble_event(ble_evt).await,
                Event::Soc(soc_evt) => self.handle_soc_event(soc_evt).await,
            }
        }
    }
}
```

### Task 2: BLE Event Processing
**File**: `src/event_handler.rs`

```rust
impl EventHandler {
    async fn handle_ble_event(&self, event: BleEvent) {
        let packet = match event {
            BleEvent::Gap(gap_evt) => self.process_gap_event(gap_evt).await,
            BleEvent::Gatts(gatts_evt) => self.process_gatts_event(gatts_evt).await,
            BleEvent::Gattc(gattc_evt) => self.process_gattc_event(gattc_evt).await,
            _ => return, // Ignore unhandled event types
        };
        
        if let Ok(pkt) = packet {
            // Send to TX channel for SPI transmission
            self.tx_channel.send(pkt).await;
        }
    }
    
    async fn process_gap_event(&self, event: GapEvent) -> Result<TxPacket, EventError> {
        match event {
            GapEvent::Connected { conn_handle, role, peer_addr, conn_params } => {
                // Special handling: must append advertising data + scan response
                let mut packet = TX_POOL.alloc()?;
                
                // Write event header
                packet.write_u16(RSP_EVENT_BLE)?;
                packet.write_u8(BLE_GAP_EVT_CONNECTED)?;
                
                // Write connection data
                packet.write_u16(conn_handle)?;
                packet.write_u8(role)?;
                packet.write_bytes(&peer_addr.addr)?;
                packet.write_u8(peer_addr.addr_type)?;
                
                // Write connection parameters
                packet.write_u16(conn_params.min_conn_interval)?;
                packet.write_u16(conn_params.max_conn_interval)?;
                packet.write_u16(conn_params.slave_latency)?;
                packet.write_u16(conn_params.conn_sup_timeout)?;
                
                // Append advertising data (critical for C firmware compatibility)
                self.append_advertising_data(&mut packet).await?;
                
                Ok(packet)
            },
            GapEvent::Disconnected { conn_handle, reason } => {
                let mut packet = TX_POOL.alloc()?;
                packet.write_u16(RSP_EVENT_BLE)?;
                packet.write_u8(BLE_GAP_EVT_DISCONNECTED)?;
                packet.write_u16(conn_handle)?;
                packet.write_u8(reason)?;
                Ok(packet)
            },
            // ... handle other GAP events
        }
    }
}
```

### Task 3: Special Event Data Handling
**File**: `src/event_handler.rs`

```rust
impl EventHandler {
    async fn append_advertising_data(&self, packet: &mut TxPacket) -> Result<(), EventError> {
        // From C firmware analysis: Connected events must include both
        // advertising data and scan response data that was active during connection
        
        // Get current advertising data from advertising manager
        let adv_data = self.get_current_adv_data().await?;
        let scan_rsp_data = self.get_current_scan_response().await?;
        
        // Write advertising data length + data
        packet.write_u8(adv_data.len() as u8)?;
        packet.write_bytes(&adv_data)?;
        
        // Write scan response data length + data  
        packet.write_u8(scan_rsp_data.len() as u8)?;
        packet.write_bytes(&scan_rsp_data)?;
        
        Ok(())
    }
    
    async fn process_gatts_event(&self, event: GattsEvent) -> Result<TxPacket, EventError> {
        match event {
            GattsEvent::Write { conn_handle, handle, offset, data } => {
                let mut packet = TX_POOL.alloc()?;
                packet.write_u16(RSP_EVENT_BLE)?;
                packet.write_u8(BLE_GATTS_EVT_WRITE)?;
                packet.write_u16(conn_handle)?;
                packet.write_u16(handle)?;
                packet.write_u16(offset)?;
                packet.write_u16(data.len() as u16)?;
                packet.write_bytes(data)?;
                Ok(packet)
            },
            GattsEvent::MtuExchange { conn_handle, client_mtu } => {
                let mut packet = TX_POOL.alloc()?;
                packet.write_u16(RSP_EVENT_BLE)?;
                packet.write_u8(BLE_GATTS_EVT_MTU_EXCHANGE)?;
                packet.write_u16(conn_handle)?;
                packet.write_u16(client_mtu)?;
                Ok(packet)
            },
            // ... handle other GATTS events
        }
    }
}
```

### Task 4: Memory-Constrained Event Buffer Management
**File**: `src/event_handler.rs`

```rust
// MEMORY CRITICAL: Minimize event queue size to fit in remaining RAM budget

pub const EVENT_QUEUE_SIZE: usize = 4; // Reduced from 16 to 4 (save ~3KB RAM)

pub struct EventQueue {
    // Use existing TX buffer pool instead of separate event buffers
    queue: Channel<NoopRawMutex, TxPacket, EVENT_QUEUE_SIZE>,
    drop_counter: AtomicU32, // 4 bytes only
}

impl EventQueue {
    pub async fn enqueue(&self, event: TxPacket) {
        // Aggressive dropping strategy for memory constraints
        match self.queue.try_send(event) {
            Ok(()) => {},
            Err(_) => {
                // Queue full - drop immediately, no retry
                self.drop_counter.fetch_add(1, Ordering::Relaxed);
                
                // CRITICAL: Return TxPacket to pool immediately
                drop(event); // This returns to TX_POOL
            }
        }
    }
    
    pub async fn dequeue(&self) -> TxPacket {
        self.queue.receive().await
    }
}

// Total memory impact: 4 * (TxPacket ref) + 4 bytes = minimal
// Reuses existing TX buffer pool - no additional allocation
```

## Data Structures

### Event Response Codes
```rust
pub const RSP_EVENT_BLE: u16 = 0x8001;
pub const RSP_EVENT_SOC: u16 = 0x8002;
```

### BLE Event IDs (from Nordic SDK)
```rust
pub const BLE_GAP_EVT_CONNECTED: u8 = 0x11;
pub const BLE_GAP_EVT_DISCONNECTED: u8 = 0x12;
pub const BLE_GAP_EVT_CONN_PARAM_UPDATE: u8 = 0x13;
pub const BLE_GAP_EVT_PHY_UPDATE: u8 = 0x18;
pub const BLE_GAP_EVT_DATA_LENGTH_UPDATE: u8 = 0x19;
pub const BLE_GAP_EVT_RSSI_CHANGED: u8 = 0x1F;

pub const BLE_GATTS_EVT_WRITE: u8 = 0x50;
pub const BLE_GATTS_EVT_MTU_EXCHANGE: u8 = 0x54;
pub const BLE_GATTS_EVT_HVN_TX_COMPLETE: u8 = 0x55;
pub const BLE_GATTS_EVT_HVC: u8 = 0x56;
```

### Event Packet Format
```rust
pub struct EventPacket {
    pub response_code: u16,    // 0x8001 for BLE, 0x8002 for SOC
    pub event_id: u8,          // Specific BLE event ID
    pub payload: Vec<u8, 240>, // Variable length payload (max ~240 bytes)
}
```

## Integration Points

### With SPI TX System
- Events must be queued for transmission over TX SPI
- Event packets use same TX buffer pool as command responses
- Priority handling: Events have lower priority than command responses

### With State Management
- Connection state must be tracked for conn_handle validation
- Advertising state needed for advertising data appending
- GATT state needed for handle-to-service/characteristic mapping

### With Buffer Pool
- Events consume TX buffers from shared pool
- Must handle buffer exhaustion gracefully
- Event dropping policy when buffers unavailable

## Testing Strategy

### Unit Tests
- Test event packet formatting for each event type
- Test buffer overflow handling
- Test advertising data appending logic
- Test event queue behavior under load

### Hardware Tests
- Verify actual BLE events trigger proper packet transmission
- Test connection event includes correct advertising data
- Test GATT write events contain correct payload data
- Test event ordering under concurrent operations

### Integration Tests
- Test event forwarding during connection establishment
- Test event flow during GATT operations
- Test system stability under event burst conditions
- Test memory usage during sustained event traffic

## Performance Considerations

### Memory Usage
- Event queue depth affects RAM usage
- Large payloads (like advertising data) consume significant buffer space
- Must balance queue depth vs memory constraints

### Timing
- Events must be processed quickly to avoid SoftDevice queue overflow
- SPI transmission rate limits event throughput
- Critical events (connection/disconnection) have timing requirements

### Priority
- Connection events have highest priority
- GATT write events need fast processing for response latency
- RSSI events can be dropped if necessary

## Error Handling

### SoftDevice Errors
- Handle event parsing failures gracefully
- Invalid connection handles in events
- Malformed event payloads from SoftDevice

### Resource Exhaustion
- TX buffer pool exhaustion during event bursts
- Event queue overflow handling
- SPI transmission backpressure

### Recovery Strategies
- Event dropping with counters for debugging
- Graceful degradation under resource pressure
- System reset as last resort for unrecoverable states

## Dependencies

### Internal
- TX buffer pool and SPI transmission system
- Advertising manager for current advertising data
- Connection state tracking
- GATT registry for handle mapping

### External
- `nrf-softdevice` event types and APIs
- `embassy-sync` for async event channels
- `heapless` for fixed-size event queues

## Risk Assessment

### Critical Risks
- **Event Loss**: Missing critical events breaks host state tracking
- **Memory Leaks**: Event buffers not returned to pool
- **Timing**: Slow event processing causes SoftDevice buffer overflow

### Medium Risks
- **Event Reordering**: Events arrive out of expected order
- **Payload Truncation**: Large events exceed buffer capacity
- **Connection Handle Mapping**: Invalid handles in events

## Migration from C Firmware
- Event format must exactly match C firmware for host compatibility
- Advertising data appending for connected events is critical
- Response codes (0x8001, 0x8002) must be preserved
- Event priority and ordering behavior should match

## Next Steps
This event system enables the host to maintain complete awareness of BLE stack state, completing the core proxy functionality needed for Phase 3D integration testing.