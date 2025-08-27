# Phase D: Event System & GAP Operations

## Overview
Complete the BLE modem by implementing the event forwarding system that captures all BLE events and forwards them to the host, plus implement comprehensive GAP (Generic Access Profile) operations for full BLE control.

**CURRENT STATUS: 10% COMPLETE** - GAP command stubs exist, no event system

## Goals
- 🔴 Capture all BLE events from SoftDevice **[TODO]**
- 🔴 Serialize and forward events to host via SPI **[TODO]**
- ⚠️ Implement complete GAP command set **[STUBS EXIST]**
- 🔴 Handle connection parameters and PHY updates **[TODO]**
- ⚠️ Support advertising configuration and control **[PARTIAL]**

## Technical Requirements

### Event Types to Forward
```
GAP Events (0x8001-0x801F):
- Connected (includes adv/scan data)
- Disconnected (with reason)
- Connection parameter update
- PHY update
- RSSI changed
- Data length update

GATTS Events (0x8020-0x803F):
- Write request/command
- Read request
- Handle value confirmation
- Service changed
- HVX complete (notification/indication sent)

System Events (0x8040-0x805F):
- Memory request
- Data overflow
- Timeout
```

### GAP Commands to Implement
```
Device Configuration (0x0030-0x003F):
- SetDeviceName, GetDeviceName
- SetAppearance, GetAppearance  
- SetPreferredConnParams
- SetTxPower, GetTxPower

Advertising Control (0x0040-0x004F):
- StartAdvertising, StopAdvertising
- SetAdvertisingData, SetScanResponseData
- SetAdvertisingParams (interval, timeout)

Connection Management (0x0050-0x005F):
- UpdateConnParams, RequestConnParamUpdate
- UpdatePHY, SetPreferredPHY
- StartRSSI, StopRSSI
- SetDataLength
- Disconnect
```

## Implementation Tasks

### 1. Event Capture System
**File: `src/events.rs`**
```rust
use nrf_softdevice::ble::evt::{BleEvent, GapEvent, GattsEvent};

#[derive(Debug, Clone)]
pub enum ModemEvent {
    // GAP Events
    Connected {
        conn_handle: u16,
        peer_addr: [u8; 6],
        role: u8,
        conn_params: ConnectionParams,
        adv_data: Vec<u8>,
        scan_data: Vec<u8>,
    },
    Disconnected {
        conn_handle: u16,
        reason: u8,
    },
    ConnParamsUpdated {
        conn_handle: u16,
        params: ConnectionParams,
    },
    PhyUpdated {
        conn_handle: u16,
        tx_phy: u8,
        rx_phy: u8,
    },
    RssiChanged {
        conn_handle: u16,
        rssi: i8,
    },
    
    // GATTS Events
    GattsWrite {
        conn_handle: u16,
        handle: u16,
        op: u8,
        data: Vec<u8>,
    },
    GattsRead {
        conn_handle: u16,
        handle: u16,
    },
    HvxComplete {
        conn_handle: u16,
        handle: u16,
    },
}

pub fn convert_softdevice_event(evt: &BleEvent) -> Option<ModemEvent> {
    match evt {
        BleEvent::Gap(gap_evt) => convert_gap_event(gap_evt),
        BleEvent::Gatts(gatts_evt) => convert_gatts_event(gatts_evt),
        _ => None,
    }
}
```

### 2. Event Forwarding Task
**File: `src/events.rs`** (continued)
```rust
#[embassy_executor::task]
pub async fn event_forwarder(
    sd: &'static Softdevice,
    tx_queue: &'static TxQueue,
    state: &'static Mutex<ModemState>,
) {
    let events = sd.events();
    
    loop {
        let evt = events.recv().await;
        
        // Convert to modem event
        if let Some(modem_evt) = convert_softdevice_event(&evt) {
            // Special handling for connection events
            if let ModemEvent::Connected { conn_handle, .. } = &modem_evt {
                let mut state = state.lock().await;
                state.connection = Some(Connection::new(*conn_handle));
                
                // Capture advertising data that was used
                if let Some(adv_handle) = state.advertising_handle {
                    modem_evt.adv_data = state.last_adv_data.clone();
                    modem_evt.scan_data = state.last_scan_data.clone();
                }
            }
            
            // Serialize event
            let packet = serialize_event(modem_evt);
            
            // Queue for SPI transmission
            tx_queue.enqueue(packet).await;
        }
    }
}

fn serialize_event(evt: ModemEvent) -> Vec<u8> {
    let mut buffer = Vec::new();
    
    // Event header
    buffer.extend_from_slice(&(0x8001u16).to_be_bytes()); // Event code
    
    // Event-specific data
    match evt {
        ModemEvent::Connected { peer_addr, adv_data, scan_data, .. } => {
            buffer.extend_from_slice(&peer_addr);
            buffer.push(adv_data.len() as u8);
            buffer.extend_from_slice(&adv_data);
            buffer.push(scan_data.len() as u8);
            buffer.extend_from_slice(&scan_data);
        }
        ModemEvent::GattsWrite { handle, data, .. } => {
            buffer.extend_from_slice(&handle.to_be_bytes());
            buffer.extend_from_slice(&(data.len() as u16).to_be_bytes());
            buffer.extend_from_slice(&data);
        }
        // ... other events
    }
    
    buffer
}
```

### 3. GAP Operations Implementation
**File: `src/commands/gap.rs`**
```rust
use nrf_softdevice::ble::{peripheral, central, gap};

pub async fn start_advertising(
    payload: &[u8],
    state: &mut ModemState,
    sd: &'static Softdevice,
) -> Result<Response, CommandError> {
    let request: StartAdvRequest = postcard::from_bytes(payload)?;
    
    // Store advertising data for event reporting
    state.last_adv_data = request.adv_data.clone();
    state.last_scan_data = request.scan_data.clone();
    
    // Configure advertising
    let config = peripheral::Config {
        interval: request.interval,
        timeout: request.timeout,
        max_events: request.max_events,
        primary_phy: convert_phy(request.primary_phy),
        secondary_phy: convert_phy(request.secondary_phy),
    };
    
    // Build advertisement
    let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
        adv_data: &request.adv_data,
        scan_data: &request.scan_data,
    };
    
    // Start advertising (non-blocking)
    let handle = peripheral::advertise_start(sd, adv, &config)?;
    state.advertising_handle = Some(handle);
    
    Ok(Response::success(CommandCode::StartAdvertising, &[]))
}

pub async fn update_conn_params(
    payload: &[u8],
    state: &mut ModemState,
    conn: &Connection,
) -> Result<Response, CommandError> {
    let request: UpdateConnParamsRequest = postcard::from_bytes(payload)?;
    
    let params = gap::ConnectionParameters {
        min_interval: request.min_interval,
        max_interval: request.max_interval,
        slave_latency: request.slave_latency,
        supervision_timeout: request.supervision_timeout,
    };
    
    gap::update_connection_parameters(conn, &params).await?;
    
    Ok(Response::success(CommandCode::UpdateConnParams, &[]))
}

pub async fn update_phy(
    payload: &[u8],
    conn: &Connection,
) -> Result<Response, CommandError> {
    let request: UpdatePhyRequest = postcard::from_bytes(payload)?;
    
    let phy_config = gap::PhyUpdateConfig {
        tx_phy: convert_to_phy(request.tx_phy),
        rx_phy: convert_to_phy(request.rx_phy),
    };
    
    gap::update_phy(conn, &phy_config).await?;
    
    Ok(Response::success(CommandCode::UpdatePHY, &[]))
}

pub async fn start_rssi_monitoring(
    payload: &[u8],
    conn: &Connection,
) -> Result<Response, CommandError> {
    let request: StartRssiRequest = postcard::from_bytes(payload)?;
    
    gap::start_rssi(conn, request.threshold, request.skip_count)?;
    
    Ok(Response::success(CommandCode::StartRSSI, &[]))
}

pub async fn set_tx_power(
    payload: &[u8],
    state: &mut ModemState,
    sd: &'static Softdevice,
) -> Result<Response, CommandError> {
    let tx_power = i8::from_be_bytes([payload[0]]);
    
    // Validate against supported levels
    let actual_power = gap::set_tx_power(sd, tx_power)?;
    state.tx_power = actual_power;
    
    let response = [actual_power as u8];
    Ok(Response::success(CommandCode::SetTxPower, &response))
}
```

### 4. Connection Management
**File: `src/connection.rs`**
```rust
pub struct ConnectionManager {
    connections: heapless::FnvIndexMap<u16, ConnectionInfo, 8>,
}

pub struct ConnectionInfo {
    conn: Connection,
    peer_addr: [u8; 6],
    conn_params: ConnectionParameters,
    phy: PhyConfig,
    rssi_monitoring: bool,
    mtu: u16,
}

impl ConnectionManager {
    pub fn on_connected(&mut self, evt: &ConnectedEvent) -> Result<(), Error> {
        let info = ConnectionInfo {
            conn: Connection::from_handle(evt.conn_handle),
            peer_addr: evt.peer_addr,
            conn_params: evt.conn_params,
            phy: PhyConfig::default(),
            rssi_monitoring: false,
            mtu: 23, // Default ATT MTU
        };
        
        self.connections.insert(evt.conn_handle, info)?;
        Ok(())
    }
    
    pub fn on_disconnected(&mut self, handle: u16) {
        self.connections.remove(&handle);
    }
    
    pub fn get_connection(&self, handle: u16) -> Option<&Connection> {
        self.connections.get(&handle).map(|info| &info.conn)
    }
}
```

### 5. Integration with Main Loop
**File: `src/main.rs`** (additions)
```rust
// Spawn event forwarder
unwrap!(spawner.spawn(event_forwarder(sd, tx_queue, state)));

// Update command processor to handle GAP commands
let response = match cmd_code {
    // ... existing commands ...
    
    // GAP commands
    CommandCode::StartAdvertising => {
        gap::start_advertising(payload, &mut state, sd).await
    }
    CommandCode::StopAdvertising => {
        gap::stop_advertising(&mut state).await
    }
    CommandCode::UpdateConnParams => {
        if let Some(conn) = state.connection.as_ref() {
            gap::update_conn_params(payload, &mut state, conn).await
        } else {
            Err(CommandError::NotConnected)
        }
    }
    // ... more GAP commands ...
};
```

## Testing Strategy

### 1. Event Capture Test
**File: `src/bin/test_events.rs`**
```rust
// Connect and verify event generation
// Test all event types
// Verify event serialization
// Check event ordering
```

### 2. GAP Operations Test
- Test advertising start/stop/configure
- Verify connection parameter updates
- Test PHY switching (1M ↔ 2M)
- Verify TX power control
- Test RSSI monitoring

### 3. Integration Test
- Full command/event flow with host
- Multiple connections (if supported)
- Stress test with rapid connect/disconnect
- Verify memory leak-free operation

## Success Criteria
1. ✅ All BLE events forwarded to host
2. ✅ GAP commands implemented and functional
3. ✅ Connection parameters controllable
4. ✅ PHY updates working (1M/2M)
5. ✅ Event serialization correct and efficient

## Technical Challenges

### Challenge: Event Buffering
- **Solution**: Use ring buffer with overflow detection

### Challenge: Event Ordering
- **Solution**: Single event task ensures FIFO ordering

### Challenge: Connection State Sync
- **Solution**: Update state atomically in event handler

## Next Phase
Phase E will focus on integration testing, performance optimization, and production readiness.