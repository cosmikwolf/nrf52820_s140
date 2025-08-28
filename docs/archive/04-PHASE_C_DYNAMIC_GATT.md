# Phase C: Dynamic GATT Services

## Overview
Implement runtime GATT service and characteristic creation using nrf-softdevice's `ServiceBuilder` API, enabling the host to dynamically define the BLE service structure via SPI commands.

**CURRENT STATUS: 0% COMPLETE** - Not started, static services exist

## Goals
- 🔴 Create dynamic GATT service builder using `ServiceBuilder` **[TODO]**
- 🔴 Implement characteristic addition with full property support **[TODO]**
- 🔴 Handle CCCD/SCCD descriptors automatically **[TODO]**
- 🔴 Manage service/characteristic handle mapping **[TODO]**
- 🔴 Implement value read/write operations **[TODO]**

## Key Discovery
The nrf-softdevice crate DOES support runtime service creation via `ServiceBuilder`! This is demonstrated in the `ble_dis_bas_peripheral_builder.rs` example. We don't need raw SoftDevice calls.

## Technical Requirements

### GATTS Commands to Implement
```
Service Management:
- AddService (0x0050): Create new service with UUID
- RemoveService (0x0051): Remove service by handle
- RemoveAllServices (0x0052): Clear all services

Characteristic Management:
- AddCharacteristic (0x0053): Add char to service
- RemoveCharacteristic (0x0054): Remove char by handle
- SetValue (0x0055): Update characteristic value
- GetValue (0x0056): Read characteristic value
- NotifyValue (0x0057): Send notification
- IndicateValue (0x0058): Send indication
```

### Handle Management
```
Service Handles: 0x0001-0x00FF (max 255 services)
Char Handles: 0x0100-0xFFFF (max 65280 characteristics)
- Value handle
- CCCD handle (if notify/indicate enabled)
- User descriptor handles
```

## Implementation Tasks

### 1. GATT Builder Module
**File: `src/gatt_builder.rs`**
```rust
use nrf_softdevice::ble::gatt_server::builder::ServiceBuilder;
use nrf_softdevice::ble::gatt_server::characteristic::{Attribute, Metadata, Properties};
use nrf_softdevice::ble::{Uuid, SecurityMode};

pub struct GattBuilder {
    services: heapless::Vec<ServiceInfo, 16>,
    characteristics: heapless::Vec<CharInfo, 64>,
}

impl GattBuilder {
    pub fn add_service(
        &mut self,
        sd: &mut Softdevice,
        uuid: Uuid,
    ) -> Result<ServiceHandle, BuilderError> {
        let mut builder = ServiceBuilder::new(sd, uuid)?;
        let handle = builder.build();
        
        self.services.push(ServiceInfo {
            handle,
            uuid,
            char_count: 0,
        })?;
        
        Ok(handle)
    }
    
    pub fn add_characteristic(
        &mut self,
        service: &mut ServiceBuilder,
        uuid: Uuid,
        props: CharProperties,
        value: &[u8],
    ) -> Result<CharHandles, BuilderError> {
        let properties = self.convert_properties(props);
        let attr = Attribute::new(value)
            .security(SecurityMode::Open);
        let metadata = Metadata::new(properties);
        
        let handles = service
            .add_characteristic(uuid, attr, metadata)?
            .build();
        
        self.characteristics.push(CharInfo {
            handles,
            uuid,
            properties: props,
            current_value: heapless::Vec::from_slice(value)?,
        })?;
        
        Ok(handles)
    }
}
```

### 2. Dynamic GATT Server
**File: `src/dynamic_server.rs`**
```rust
use nrf_softdevice::ble::gatt_server::{self, Server, WriteOp};

pub struct DynamicGattServer {
    builder: GattBuilder,
    write_callback: Option<WriteCallback>,
}

impl Server for DynamicGattServer {
    type Event = GattEvent;
    
    fn on_write(
        &self,
        conn: &Connection,
        handle: u16,
        op: WriteOp,
        offset: usize,
        data: &[u8],
    ) -> Option<Self::Event> {
        // Find characteristic by handle
        if let Some(char_info) = self.find_char_by_handle(handle) {
            // Update internal value
            char_info.current_value.clear();
            char_info.current_value.extend_from_slice(data).ok()?;
            
            // Generate event for host
            Some(GattEvent::Write {
                conn_handle: conn.handle(),
                char_handle: handle,
                data: data.to_vec(),
            })
        } else {
            None
        }
    }
    
    fn on_read(
        &self,
        conn: &Connection,
        handle: u16,
    ) -> Option<&[u8]> {
        // Return current value from our cache
        self.find_char_by_handle(handle)
            .map(|char| char.current_value.as_slice())
    }
}
```

### 3. GATTS Command Handlers
**File: `src/commands/gatts.rs`**
```rust
pub async fn add_service(
    payload: &[u8],
    state: &mut ModemState,
    sd: &'static Softdevice,
) -> Result<Response, CommandError> {
    let request: AddServiceRequest = postcard::from_bytes(payload)?;
    let uuid = resolve_uuid(&request.uuid, &state.uuid_bases)?;
    
    let handle = state.gatt_builder
        .add_service(sd, uuid)?;
    
    let response = AddServiceResponse {
        service_handle: handle.as_u16(),
    };
    
    Ok(Response::success(
        CommandCode::AddService,
        postcard::to_slice(&response)?,
    ))
}

pub async fn add_characteristic(
    payload: &[u8],
    state: &mut ModemState,
    sd: &'static Softdevice,
) -> Result<Response, CommandError> {
    let request: AddCharRequest = postcard::from_bytes(payload)?;
    
    // Find service builder
    let service = state.gatt_builder
        .get_service_mut(request.service_handle)?;
    
    let uuid = resolve_uuid(&request.uuid, &state.uuid_bases)?;
    let handles = state.gatt_builder.add_characteristic(
        service,
        uuid,
        request.properties,
        &request.initial_value,
    )?;
    
    let response = AddCharResponse {
        value_handle: handles.value,
        cccd_handle: handles.cccd,
    };
    
    Ok(Response::success(
        CommandCode::AddCharacteristic,
        postcard::to_slice(&response)?,
    ))
}

pub async fn set_value(
    payload: &[u8],
    state: &mut ModemState,
    conn: Option<&Connection>,
) -> Result<Response, CommandError> {
    let request: SetValueRequest = postcard::from_bytes(payload)?;
    
    // Update internal cache
    state.gatt_builder
        .update_value(request.handle, &request.value)?;
    
    // Update SoftDevice if connected
    if let Some(conn) = conn {
        gatt_server::set_value(
            sd,
            request.handle,
            &request.value,
        )?;
    }
    
    Ok(Response::success(CommandCode::SetValue, &[]))
}

pub async fn notify_value(
    payload: &[u8],
    state: &mut ModemState,
    conn: &Connection,
) -> Result<Response, CommandError> {
    let request: NotifyRequest = postcard::from_bytes(payload)?;
    
    // Send notification
    gatt_server::notify_value(
        conn,
        request.handle,
        &request.value,
    ).await?;
    
    Ok(Response::success(CommandCode::NotifyValue, &[]))
}
```

### 4. UUID Resolution
**File: `src/commands/uuid.rs`** (additions)
```rust
pub fn resolve_uuid(
    uuid_ref: &UuidRef,
    bases: &[Uuid128],
) -> Result<Uuid, CommandError> {
    match uuid_ref {
        UuidRef::Uuid16(val) => Ok(Uuid::new_16(*val)),
        UuidRef::Uuid128(bytes) => Ok(Uuid::new_128(bytes)),
        UuidRef::BaseOffset { base_idx, offset } => {
            let base = bases.get(*base_idx as usize)
                .ok_or(CommandError::InvalidUuidBase)?;
            Ok(Uuid::new_128_from_base(base, *offset))
        }
    }
}
```

### 5. Integration with Main Server
**File: `src/main.rs`** (modifications)
```rust
// Replace static Server with DynamicGattServer
let mut gatt_server = DynamicGattServer::new();

// In ble_task, use dynamic server
let e = gatt_server::run(&conn, &gatt_server, |event| {
    match event {
        GattEvent::Write { handle, data, .. } => {
            // Forward to host via SPI event system (Phase D)
            send_gatt_event(handle, data).await;
        }
        _ => {}
    }
}).await;
```

## Testing Strategy

### 1. Service Creation Test
**File: `src/bin/test_dynamic_gatt.rs`**
```rust
// Test creating services at runtime
// Add multiple characteristics
// Verify handle allocation
// Test service removal
```

### 2. Characteristic Operations
- Test all property combinations (read, write, notify, indicate)
- Verify CCCD handling for notifications
- Test value updates and caching
- Verify maximum characteristic limits

### 3. Host Integration Test
- Create complex GATT structure via SPI commands
- Connect with BLE client and verify structure
- Test notifications/indications
- Verify write callbacks reach host

## Success Criteria
1. ✅ Services can be created/removed at runtime
2. ✅ Characteristics support all standard properties
3. ✅ Notifications/indications work correctly
4. ✅ Value caching maintains consistency
5. ✅ Handle mapping is accurate and persistent

## Technical Challenges

### Challenge: Dynamic vs Static Trade-off
- **Solution**: Use `ServiceBuilder` for runtime creation, maintain handle cache

### Challenge: CCCD State Management
- **Solution**: Let nrf-softdevice handle CCCD internally, we just track handles

### Challenge: Memory Limits
- **Solution**: Pre-allocate pools, return errors when exhausted

## Next Phase
Phase D will implement the event forwarding system and complete GAP operations, allowing full BLE modem functionality.