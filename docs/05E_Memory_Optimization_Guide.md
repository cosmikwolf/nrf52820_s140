# Phase 3E: Memory Optimization & Monitoring Guide

## Overview
Critical guidance for staying within nRF52820's 16KB RAM constraint. This phase must be executed **concurrently** with all implementation phases.

## Memory Budget Breakdown

### Absolute Limits (16KB RAM Total)
```
SoftDevice Reserved:     16KB (unmovable)
Application Available:   16KB

Current Usage:          ~10-11KB
Remaining Budget:        5-6KB
Target Complete Usage:  <15KB (1KB safety margin)
```

### Per-Phase Memory Allocation
| Phase | Component | Budget | Critical Monitors |
|-------|-----------|--------|-------------------|
| **Current** | TX Pool + Embassy + Stack | 10-11KB | Fixed baseline |
| **3A GAP** | State + Config | +1.2KB | Device name, advertising data |
| **3B GATT** | Service Registry | +1.0KB | Service/char tables |
| **3C Events** | Event Queue + State | +0.8KB | Event buffers, connection tracking |
| **Safety** | Margin | 1.0KB | Prevent overflow |
| **Total** | | **15.0KB** | **Must not exceed** |

## Mandatory Compiler Optimizations

### Cargo.toml Configuration
```toml
[profile.release]
opt-level = "z"           # Optimize for size (not speed)
lto = true               # Link-time optimization
codegen-units = 1        # Reduce code duplication
panic = "abort"          # Remove unwinding overhead
strip = true             # Remove debug symbols
overflow-checks = false  # Disable runtime checks

[profile.dev]
opt-level = 1            # Some optimization even in dev
overflow-checks = false  # Critical for space
```

### Embassy Task Configuration
```rust
// Reduce all task stack sizes to minimum viable
#[embassy_executor::task(pool_size = 1)]
async fn softdevice_task() { /* 1KB default */ }

#[embassy_executor::task(pool_size = 1, task_stack_size = 512)]
async fn spi_tx_task() { /* 512 bytes */ }

#[embassy_executor::task(pool_size = 1, task_stack_size = 512)] 
async fn spi_rx_task() { /* 512 bytes */ }

#[embassy_executor::task(pool_size = 1, task_stack_size = 768)]
async fn command_processor() { /* 768 bytes */ }

#[embassy_executor::task(pool_size = 1, task_stack_size = 512)]
async fn event_handler() { /* 512 bytes */ }

// Total task stacks: ~3.3KB (down from 5-6KB)
```

## Memory Monitoring Strategy

### Continuous Monitoring Tools
```bash
# After each code change, run these commands:

# 1. Check binary size breakdown
cargo bloat --release --crates

# 2. Check total memory usage  
cargo size --release

# 3. Check per-symbol memory usage
cargo bloat --release -n 50

# 4. Monitor specific large symbols
cargo bloat --release --filter embassy
```

### Memory Usage Targets
```bash
# Current baseline (must not regress):
text: ~31KB  (flash)
data: ~84B   (flash)
bss:  ~9.6KB (RAM static)

# Phase 3A target:
bss: <11KB   (additional 1.2KB)

# Phase 3B target:  
bss: <12KB   (additional 1.0KB)

# Phase 3C target:
bss: <13KB   (additional 0.8KB)
```

### Runtime Memory Analysis
```rust
// Add to main.rs for development builds
#[cfg(debug_assertions)]
fn report_memory_usage() {
    extern "C" {
        static mut _sbss: u32;
        static mut _ebss: u32;
        static mut _sdata: u32;
        static mut _edata: u32;
    }
    
    unsafe {
        let bss_size = &_ebss as *const u32 as usize - &_sbss as *const u32 as usize;
        let data_size = &_edata as *const u32 as usize - &_sdata as *const u32 as usize;
        defmt::info!("Memory usage - BSS: {}KB, DATA: {}KB", 
                    bss_size / 1024, data_size / 1024);
    }
}
```

## Critical Memory Anti-Patterns

### ❌ NEVER DO THESE
```rust
// 1. Don't use Vec or HashMap
use std::collections::HashMap; // FORBIDDEN
let mut map = HashMap::new();  // FORBIDDEN

// 2. Don't use String
use std::string::String; // FORBIDDEN
let name = String::from("device"); // FORBIDDEN

// 3. Don't use large stack arrays
fn process_data() {
    let buffer = [0u8; 1024]; // DANGEROUS - 1KB on stack
}

// 4. Don't allocate in async tasks
async fn task() {
    let data = vec![0; 100]; // FORBIDDEN
}
```

### ✅ ALWAYS DO THESE
```rust
// 1. Use heapless collections with explicit limits
use heapless::{FnvIndexMap, String};
let mut map: FnvIndexMap<u16, ServiceInfo, 8> = FnvIndexMap::new();

// 2. Use heapless::String with size limits
let mut name: heapless::String<32> = heapless::String::new();

// 3. Use static allocation
static BUFFER: [u8; 1024] = [0; 1024]; // Counted in BSS

// 4. Use #[repr(packed)] for structs
#[repr(packed)]
struct CompactData {
    field1: u16,
    field2: u8,
} // 3 bytes instead of 4
```

## Phase-Specific Optimization Requirements

### Phase 3A: GAP Operations
```rust
// Memory-efficient device configuration
#[repr(packed)]
struct DeviceConfig {
    name: [u8; 32],      // Fixed size, no String
    address: [u8; 6],    // Packed address
    addr_type: u8,       // Single byte
    tx_power: i8,        // Single byte
    flags: u16,          // Packed boolean flags
} // Total: 42 bytes

// Static allocation instead of dynamic
static mut DEVICE_CONFIG: DeviceConfig = DeviceConfig::new();
```

### Phase 3B: GATT Foundation
```rust
// Array-based registry instead of HashMap
struct GattRegistry {
    services: [ServiceInfo; 8],           // 8 * 8 = 64 bytes
    characteristics: [CharacteristicInfo; 32], // 32 * 12 = 384 bytes
    service_count: u8,
    char_count: u8,
} // Total: ~450 bytes vs 2KB+ for HashMap

// Bit-packed UUIDs
#[repr(packed)]
struct PackedUuid {
    data: u32,       // Encoded UUID data
    uuid_type: u8,   // Type discriminant
} // 5 bytes vs 18 bytes for full UUID struct
```

### Phase 3C: Event System
```rust
// Minimal event queue - reuse TX buffers
struct EventSystem {
    queue: Channel<NoopRawMutex, TxPacket, 4>, // 4 slots only
    drop_count: AtomicU32,                     // 4 bytes
    state_flags: AtomicU8,                     // 1 byte
} // Total: ~20 bytes + queue overhead

// No separate event buffers - reuse TX pool
```

## Emergency Memory Recovery

### Level 1: Code Optimization
- Remove unused features from dependencies
- Disable debug formatting in release
- Optimize data structure alignment

### Level 2: Feature Reduction  
- Reduce service/characteristic limits
- Simplify UUID handling
- Remove optional command support

### Level 3: Architecture Changes
- Combine async tasks
- Move complexity to host processor
- Simplify event system

### Level 4: Hardware Consideration
- Evaluate nRF52832 (64KB RAM) upgrade
- Consider external memory
- Redesign for memory-rich target

## Success Criteria

### Mandatory Checkpoints
- [ ] Phase 3A: Total RAM < 12KB
- [ ] Phase 3B: Total RAM < 13KB  
- [ ] Phase 3C: Total RAM < 14KB
- [ ] Final: Total RAM < 15KB with 1KB safety margin

### Performance Requirements
- [ ] No memory leaks detectable over 1-hour operation
- [ ] No stack overflows under normal operation
- [ ] Event processing maintains <5ms latency
- [ ] Memory fragmentation < 100 bytes

## Monitoring Dashboard

Create a simple script for continuous monitoring:

```bash
#!/bin/bash
# memory_check.sh - Run after each commit

echo "=== Memory Usage Report ==="
echo "Flash Usage:"
cargo size --release | grep text | awk '{print "  Text: " $1/1024 "KB"}'

echo "RAM Usage:"  
cargo size --release | grep bss | awk '{print "  BSS: " $3/1024 "KB"}'

echo "Top Memory Consumers:"
cargo bloat --release -n 10

echo "Critical Check:"
BSS_SIZE=$(cargo size --release | grep nrf52820 | awk '{print $3}')
if [ $BSS_SIZE -gt 14336 ]; then  # 14KB in bytes
    echo "❌ CRITICAL: RAM usage exceeds 14KB limit!"
    exit 1
else
    echo "✅ RAM usage within limits"
fi
```

## Integration with Development Workflow

1. **Pre-commit**: Run memory_check.sh
2. **CI/CD**: Fail builds that exceed memory limits
3. **PR Reviews**: Include memory impact assessment
4. **Testing**: Monitor memory usage during hardware tests

This optimization guide must be followed religiously to ensure the implementation stays within nRF52820's constraints.