# ServiceBuilder Implementation Recommendation

**Date**: 2025-01-29  
**Status**: Architecture Decision Document  
**Decision**: Use Pre-allocated Service Pool with ServiceBuilder  

## Executive Summary

After analyzing the original C firmware and evaluating two potential approaches for dynamic GATT service creation, **the pre-allocated service pool approach using ServiceBuilder is recommended** over raw SoftDevice API calls. This maintains Rust's safety guarantees while providing the dynamic service creation functionality required by the BLE modem protocol.

## Problem Statement

The nRF52820 BLE modem firmware requires dynamic GATT service creation at runtime in response to protocol commands, but nrf-softdevice's `ServiceBuilder` requires `&mut Softdevice` access during initialization, while runtime command handlers only have `&Softdevice` access.

## Evaluated Approaches

### Approach 1: Raw SoftDevice API Calls ❌ **NOT RECOMMENDED**

**Concept**: Bypass ServiceBuilder entirely and call raw SoftDevice C APIs directly, like the original C firmware.

#### Implementation Pattern:
```rust
pub async fn handle_service_add(payload: &[u8], sd: &Softdevice) -> Result<TxPacket, CommandError> {
    let mut service_handle: u16 = 0;
    let result = unsafe {
        nrf_softdevice::raw::sd_ble_gatts_service_add(
            service_type_raw,
            uuid.as_raw_ptr(),
            &mut service_handle as *mut u16,
        )
    };
    // Manual error handling, pointer management, etc.
}
```

#### Analysis:

**Pros**:
- ✅ Directly matches original C firmware approach
- ✅ No ServiceBuilder lifecycle constraints  
- ✅ Can be called at runtime with `&Softdevice`
- ✅ Proven to work (C firmware demonstrates this)

**Cons**:
- ❌ **Extensive unsafe code required** - raw pointer management, manual memory handling
- ❌ **Loses Rust safety guarantees** - no compile-time validation, easy to introduce memory bugs
- ❌ **Complex protocol conversion** - 60+ lines of nested structure parsing (see C code lines 495-562)
- ❌ **Defeats nrf-softdevice design** - bypasses the safe abstractions the crate was built to provide
- ❌ **High maintenance burden** - unsafe code harder to review, debug, and maintain
- ❌ **Fragile to SoftDevice updates** - raw APIs more likely to break with version changes
- ❌ **Manual struct marshalling** - complex conversion between Rust types and C structures

#### Verdict: **NOT RECOMMENDED**
While technically feasible, this approach abandons Rust's core value propositions and creates a maintenance nightmare.

### Approach 2: Pre-allocated Service Pool with ServiceBuilder ✅ **RECOMMENDED**

**Concept**: Create a fixed pool of services during initialization using ServiceBuilder safely, then activate/configure them dynamically at runtime.

#### Implementation Pattern:
```rust
// During initialization (main.rs) with &mut Softdevice
pub fn initialize_service_pool(sd: &mut Softdevice) -> ServicePool {
    let mut services = Vec::new();
    for i in 0..MAX_SERVICES {
        let generic_uuid = Uuid::new_16(0x1800 + i);
        let mut service_builder = ServiceBuilder::new(sd, generic_uuid)?;
        
        // Pre-allocate characteristics with full capabilities
        for j in 0..MAX_CHARACTERISTICS {
            let char_uuid = Uuid::new_16(0x2A00 + j);
            let attr = Attribute::new(&[0u8; 64]); // Max size buffer
            let metadata = Metadata::new(
                Properties::new().read().write().notify().indicate()
            );
            let handles = service_builder.add_characteristic(char_uuid, attr, metadata)?.build();
            // Store handles
        }
        
        let service_handle = service_builder.build();
        services.push(ServiceSlot { service_handle, ... });
    }
    ServicePool { services }
}

// During runtime - activate pre-allocated services
pub async fn activate_service(uuid: BleUuid, service_type: ServiceType) -> Result<u16, ServiceError> {
    // Find unused slot in pool
    // Update registry with logical UUID mapping
    // Use gatt_server::set_value() to configure characteristics
    // Return real SoftDevice handle
}
```

#### Analysis:

**Pros**:
- ✅ **Maintains Rust safety** - no unsafe code, compile-time validation
- ✅ **Uses intended abstractions** - leverages nrf-softdevice's design properly
- ✅ **Type safety** - ServiceBuilder provides compile-time validation
- ✅ **Memory safety** - no raw pointer management
- ✅ **Maintainable** - clean, reviewable code using established patterns
- ✅ **Runtime dynamic feel** - services can be activated/deactivated on demand
- ✅ **Real SoftDevice services** - creates actual BLE services clients can discover
- ✅ **Proven pattern** - follows nrf-softdevice example architectures

**Cons**:
- ❌ **Fixed service limit** - predetermined maximum number of services (4-8)
- ❌ **Memory overhead** - pre-allocated unused services consume ~1KB RAM
- ❌ **UUID mapping complexity** - registry must translate between logical and SoftDevice UUIDs
- ❌ **Implementation effort** - requires architectural changes to service manager

#### Verdict: **RECOMMENDED**
This approach maintains Rust's safety principles while providing the required functionality.

## Technical Implementation Plan

### Phase 1: Service Pool Infrastructure
1. **Update `src/main.rs`**:
   - Add service pool initialization during startup with `&mut Softdevice`
   - Create 4-8 generic services with 4-8 characteristics each
   - Store ServiceHandles and CharacteristicHandles

2. **Rewrite `src/ble/manager.rs`**:
   - Replace channel-based system with pool-based activation
   - Implement `activate_service()` and `activate_characteristic()` functions
   - Handle slot allocation and deallocation

3. **Enhance `src/ble/registry.rs`**:
   - Add UUID translation between logical UUIDs and SoftDevice UUIDs
   - Support handle mapping for activated vs unused slots

### Phase 2: Integration
4. **Update `src/ble/services.rs`**:
   - Modify `Server::new()` to accept pre-allocated service pool
   - Integrate with existing event handling system

5. **Update command handlers**:
   - `src/commands/gatts.rs` - use pool activation instead of creation
   - Return real handles instead of placeholders

### Phase 3: Runtime Operations
6. **Characteristic value updates**:
   - Use `gatt_server::set_value()` for dynamic value changes
   - Implement proper notification/indication support

## Memory Impact Analysis

### Pre-allocated Service Pool (4 services × 8 characteristics):
- **ServiceHandles**: 4 × 8 bytes = 32 bytes
- **CharacteristicHandles**: 32 × 16 bytes = 512 bytes  
- **Pool management structures**: ~200 bytes
- **Registry UUID mapping**: ~200 bytes
- **Total overhead**: ~944 bytes

**Assessment**: Acceptable within 5KB RAM budget (16KB total - 11KB current usage).

## Risk Assessment

### Low Risk Factors:
- ✅ **Proven pattern** - ServiceBuilder usage matches nrf-softdevice examples
- ✅ **Incremental implementation** - can be built and tested in phases
- ✅ **Existing test coverage** - current test suite validates the changes

### Mitigation Strategies:
- **Service limits**: Document maximum service/characteristic counts clearly
- **Memory monitoring**: Track RAM usage during development
- **Fallback plan**: If pool approach fails, raw APIs remain as backup option

## Alternative Considerations

### Why Not Hybrid Approach?
A hybrid approach (ServiceBuilder for some services, raw APIs for others) would create inconsistency and complexity without meaningful benefits.

### Why Not Delayed Initialization?
Delaying service creation until first command would still face the `&mut Softdevice` access problem in the Embassy task system.

### Why Not Fork nrf-softdevice?
Forking would create long-term maintenance burden and compatibility issues with upstream updates.

## Decision Matrix

| Factor | Raw APIs | Service Pool |
|--------|----------|--------------|
| Safety | ❌ Unsafe | ✅ Safe |
| Maintainability | ❌ Complex | ✅ Clean |
| Performance | ✅ Direct | ✅ Efficient |
| Memory Usage | ✅ Minimal | 🟡 +1KB |
| Implementation Time | 🟡 Medium | 🟡 Medium |
| Rust Idiomatic | ❌ No | ✅ Yes |
| Future-proof | ❌ Fragile | ✅ Stable |

## Conclusion

**The pre-allocated service pool approach with ServiceBuilder is the clear winner** for implementing dynamic GATT service creation. While it requires some upfront architectural work and consumes additional memory, it maintains Rust's safety guarantees, uses the intended abstractions, and provides a maintainable long-term solution.

The raw API approach, while technically feasible, would compromise the core benefits of using Rust and nrf-softdevice. The C firmware's approach works in C because that's the only option available, but Rust provides better tools that should be used properly.

## Implementation Priority

**Status**: Ready for implementation  
**Estimated effort**: 4-6 hours  
**Risk level**: Low  
**Recommended timeline**: Implement in next development phase  

---

*This decision prioritizes long-term code quality, safety, and maintainability over short-term implementation convenience.*