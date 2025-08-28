# Phase B: Command Processing Infrastructure

## Overview
Build the command processing layer that receives SPI messages, routes them to appropriate handlers, manages modem state, and sends responses back to the host.

**CURRENT STATUS: 60% COMPLETE** - Framework exists, command handlers need implementation

## Goals
- ✅ Implement complete protocol command/response definitions **[DONE]**
- ✅ Create async command processor with routing logic **[DONE]**
- ✅ Establish modem state management **[DONE]**
- ⚠️ Implement core system commands **[PARTIAL]**
- 🔴 Verify command-response flow with host **[TODO]**

## Technical Requirements

### Command Categories
```
System Commands (0x0001-0x000F):
- GetInfo, GetVersion, Shutdown, Reboot

UUID Commands (0x0010-0x001F):
- RegisterUuidBase, RemoveUuidBase, RemoveAllUuidBases

GAP Commands (0x0030-0x004F):
- SetDeviceName, SetTxPower, StartAdvertising, etc.

GATTS Commands (0x0050-0x006F):
- AddService, AddCharacteristic, SetValue, NotifyValue, etc.
```

### Response Format
```
Success: [Length:2][0xAC50:2][CommandEcho:2][Payload:N][CRC16:2]
Error:   [Length:2][0xDEAD:2][CommandEcho:2][ErrorCode:2][CRC16:2]
Event:   [Length:2][0x8xxx:2][EventData:N][CRC16:2]
```

## Implementation Tasks

### 1. Protocol Definitions
**File: `src/protocol.rs`** ✅ **COMPLETE**
```rust
#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum CommandCode {
    // System (0x0001-0x000F)
    GetInfo = 0x0001,
    GetVersion = 0x0002,
    Shutdown = 0x0003,
    Echo = 0x0004,
    GetTemperature = 0x0005,
    Reboot = 0x0006,
    
    // UUID Management (0x0010-0x001F)
    RegisterUuidBase = 0x0010,
    RemoveUuidBase = 0x0011,
    RemoveAllUuidBases = 0x0012,
    
    // GAP Operations (0x0030-0x004F)
    SetDeviceName = 0x0030,
    GetDeviceName = 0x0031,
    SetAppearance = 0x0032,
    // ... etc
}

#[repr(u16)]
pub enum ResponseCode {
    Ack = 0xAC50,
    Error = 0xDEAD,
    BleEvent = 0x8001,
    SocEvent = 0x8002,
}
```

### 2. State Management
**File: `src/state.rs`** ✅ **COMPLETE**
```rust
pub struct ModemState {
    // System state
    pub device_name: heapless::String<31>,
    pub appearance: u16,
    pub tx_power: i8,
    
    // UUID management
    pub uuid_bases: heapless::Vec<Uuid128, 4>,
    pub uuid_map: heapless::FnvIndexMap<u16, Uuid128, 16>,
    
    // Connection state
    pub advertising_handle: Option<AdvHandle>,
    pub connection: Option<Connection>,
    pub conn_params: ConnectionParams,
    
    // GATT structure (prepared for Phase C)
    pub services: heapless::Vec<ServiceInfo, 16>,
    pub characteristics: heapless::Vec<CharInfo, 64>,
}
```

### 3. Command Router
**File: `src/commands/mod.rs`** ✅ **COMPLETE - Framework exists**
```rust
pub async fn route_command(
    cmd: CommandCode,
    payload: &[u8],
    state: &mut ModemState,
    sd: &'static Softdevice,
) -> Result<Response, CommandError> {
    match cmd {
        // System commands
        CommandCode::GetInfo => system::get_info(state).await,
        CommandCode::GetVersion => system::get_version().await,
        CommandCode::Echo => system::echo(payload).await,
        
        // UUID commands
        CommandCode::RegisterUuidBase => uuid::register_base(payload, state).await,
        
        // GAP commands (stub for now, full impl in Phase D)
        CommandCode::SetDeviceName => gap::set_device_name(payload, state, sd).await,
        
        // GATTS commands (stub for now, full impl in Phase C)
        CommandCode::AddService => gatts::add_service_stub(payload, state).await,
        
        _ => Err(CommandError::UnsupportedCommand),
    }
}
```

### 4. System Commands Implementation
**File: `src/commands/system.rs`** 🔴 **TODO - Needs implementation**
```rust
pub async fn get_info(state: &ModemState) -> Result<Response, CommandError> {
    let info = InfoResponse {
        protocol_version: 0x0100,
        firmware_version: env!("CARGO_PKG_VERSION"),
        max_connections: 1,
        max_att_mtu: 247,
        manufacturer: "Nordic",
        model: "nRF52820",
    };
    Ok(Response::success(CommandCode::GetInfo, info.serialize()))
}

pub async fn get_version() -> Result<Response, CommandError> {
    Ok(Response::success(
        CommandCode::GetVersion,
        env!("CARGO_PKG_VERSION").as_bytes(),
    ))
}

pub async fn echo(payload: &[u8]) -> Result<Response, CommandError> {
    Ok(Response::success(CommandCode::Echo, payload))
}
```

### 5. UUID Management
**File: `src/commands/uuid.rs`** 🔴 **TODO - Needs implementation**
```rust
pub async fn register_base(
    payload: &[u8],
    state: &mut ModemState,
) -> Result<Response, CommandError> {
    if payload.len() != 16 {
        return Err(CommandError::InvalidLength);
    }
    
    let uuid = Uuid128::from_slice(payload);
    let handle = state.uuid_bases.len() as u16;
    
    state.uuid_bases.push(uuid)
        .map_err(|_| CommandError::ResourceExhausted)?;
    
    let response = handle.to_be_bytes();
    Ok(Response::success(CommandCode::RegisterUuidBase, &response))
}
```

### 6. Command Processor Task
**File: `src/commands/mod.rs`** ✅ **COMPLETE - Task exists**
```rust
#[embassy_executor::task]
async fn command_processor(
    cmd_queue: &'static CmdQueue,
    tx_queue: &'static TxQueue,
    state: &'static Mutex<ModemState>,
    sd: &'static Softdevice,
) {
    loop {
        // Wait for command from SPI RX
        let cmd_frame = cmd_queue.dequeue().await;
        
        // Parse command
        let (cmd_code, payload) = parse_command(&cmd_frame);
        
        // Process command
        let response = {
            let mut state = state.lock().await;
            route_command(cmd_code, payload, &mut state, sd).await
        };
        
        // Queue response for TX
        let response_frame = build_response_frame(response);
        tx_queue.enqueue(response_frame).await;
    }
}
```

## Testing Strategy

### 1. Command Test Binary
**File: `src/bin/test_commands.rs`**
```rust
// Test each command category:
// - System: GetInfo, GetVersion, Echo
// - UUID: Register, Remove, List
// - State persistence across commands
```

### 2. Host Integration Test
- Send commands via SPI from host
- Verify correct responses
- Test command queuing under load
- Verify state consistency

### 3. Error Handling Tests
- Invalid command codes
- Malformed payloads
- Resource exhaustion
- Command timeout handling

## Dependencies
```toml
# Cargo.toml additions
heapless = { version = "0.8", features = ["fnv"] }
postcard = "1.0"  # For serialization
embassy-sync = "0.6"  # For Mutex and Channel
```

## Success Criteria
1. ✅ All system commands implemented and tested
2. ✅ UUID management working correctly
3. ✅ Command routing with < 1ms latency
4. ✅ State persistence across commands
5. ✅ Error responses for invalid commands

## Integration Points
- Receives commands from Phase A SPI layer
- Sends responses through Phase A SPI layer
- Prepares state for Phase C GATT operations
- Stubs ready for Phase D GAP operations

## Next Phase
Phase C will implement dynamic GATT service creation using the command infrastructure established here.