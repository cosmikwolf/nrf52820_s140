# BLE Modem Firmware Analysis

## Executive Summary
The analyzed C firmware implements a **Bluetooth LE modem/proxy device** that acts as a bridge between a host processor and the Nordic nRF52 BLE stack. The device **does NOT implement any actual GATT services itself**, but instead provides a comprehensive command-response interface over dual SPI channels, allowing the host to fully control all BLE operations including GATT server creation, advertising, connections, and data transfer.

## Architecture Overview

### Core Design Pattern
- **BLE Proxy/Passthrough Architecture**: The firmware acts as a transparent proxy
- **No Built-in GATT Services**: All BLE services are dynamically created by host commands
- **Dual SPI Communication**: Separate TX and RX SPI interfaces for bidirectional communication
- **Event-Driven Model**: All BLE events are forwarded to the host processor
- **Power Management**: DC/DC converter enabled, constant latency mode for predictable timing

## Communication Protocol

### Physical Layer: Dual SPI Configuration

#### TX SPI (SPIM0 - Master)
- **Purpose**: Device → Host communication
- **Pins**:
  - SS: GPIO 1
  - SCK: GPIO 0  
  - MOSI: GPIO 4
- **Speed**: 8 MHz
- **Mode**: CPOL=High, CPHA=Leading, MSB First

#### RX SPI (SPIS1 - Slave)
- **Purpose**: Host → Device communication
- **Pins**:
  - SS: GPIO 7 (with pull-up)
  - SCK: GPIO 6
  - MOSI: GPIO 5
- **Mode**: CPOL=High, CPHA=Leading, MSB First

### Message Protocol

#### Request Format (Host → Device)
```
[Payload Data (variable length)] [Request Code (2 bytes, big-endian)]
```

#### Response Format (Device → Host)
```
[Response Code (2 bytes)] [Payload Data (variable length)]
```

#### Response Codes
- `0xAC50`: ACK - Command acknowledgment with results
- `0x8001`: BLE Event - Asynchronous BLE event notification
- `0x8002`: SOC Event - System-on-Chip event notification

## Bluetooth LE Functionality

### Configuration
- **Role**: Peripheral only (Central support ifdef'd out but not enabled)
- **Connections**: 1 simultaneous connection
- **MTU**: 247 bytes maximum
- **GAP Event Length**: 24 * 1.25ms = 30ms
- **GATTS Attribute Table**: 1408 bytes
- **VS UUID Count**: 4 vendor-specific UUID bases
- **Clock Source**: Synthesized (20 PPM accuracy, no external crystal)
- **Service Changed**: Enabled

### Supported BLE Operations

#### 1. System Commands
- `REQ_GET_INFO (0x0001)`: Get firmware version (returns BCD format)
- `REQ_SHUTDOWN (0x0002)`: Power down system
- `REQ_REBOOT (0x00F0)`: System reset

#### 2. UUID Management
- `REQ_REGISTER_UUID_GROUP (0x0010)`: Register 128-bit vendor-specific UUID base

#### 3. GAP Operations
##### Address Management
- `REQ_GAP_GET_ADDR (0x0011)`: Get device BLE address
- `REQ_GAP_SET_ADDR (0x0012)`: Set device BLE address

##### Advertising Control
- `REQ_GAP_ADV_START (0x0020)`: Start advertising
- `REQ_GAP_ADV_STOP (0x0021)`: Stop advertising
- `REQ_GAP_ADV_SET_CONFIGURE (0x0022)`: Configure advertising data and parameters

##### Device Configuration
- `REQ_GAP_GET_NAME (0x0023)`: Get device name
- `REQ_GAP_SET_NAME (0x0024)`: Set device name
- `REQ_GAP_CONN_PARAMS_GET (0x0025)`: Get preferred connection parameters
- `REQ_GAP_CONN_PARAMS_SET (0x0026)`: Set preferred connection parameters

##### Connection Management
- `REQ_GAP_CONN_PARAM_UPDATE (0x0027)`: Update connection parameters
- `REQ_GAP_DATA_LENGTH_UPDATE (0x0028)`: Update data length extension
- `REQ_GAP_PHY_UPDATE (0x0029)`: Update PHY (1M/2M/Coded)
- `REQ_GAP_DISCONNECT (0x002C)`: Disconnect from peer

##### Power & RSSI
- `REQ_GAP_SET_TX_POWER (0x002D)`: Set transmit power
- `REQ_GAP_START_RSSI_REPORTING (0x002E)`: Start RSSI monitoring
- `REQ_GAP_STOP_RSSI_REPORTING (0x002F)`: Stop RSSI monitoring

#### 4. GATT Server Operations
- `REQ_GATTS_SERVICE_ADD (0x0080)`: Add GATT service (Primary/Secondary)
- `REQ_GATTS_CHARACTERISTIC_ADD (0x0081)`: Add characteristic with full metadata support:
  - Characteristic metadata (properties, user descriptor, presentation format)
  - CCCD (Client Characteristic Configuration Descriptor) metadata
  - SCCD (Server Characteristic Configuration Descriptor) metadata
  - Attribute metadata (read/write permissions, variable length)
- `REQ_GATTS_MTU_REPLY (0x0082)`: Reply to MTU exchange request
- `REQ_GATTS_HVX (0x0083)`: Send notification/indication with blocking option
- `REQ_GATTS_SYS_ATTR_GET (0x0084)`: Get system attributes (commented out/not implemented)
- `REQ_GATTS_SYS_ATTR_SET (0x0085)`: Set system attributes (for bonding)

#### 5. Central Operations (Conditionally Compiled)
The code includes support for Central role operations but these are **disabled** in the current build (`NRF_SDH_BLE_CENTRAL_LINK_COUNT = 0`):
- `REQ_GAP_CONNECT (0x002A)`: Connect to peripheral (ifdef CENTRAL)
- `REQ_GAP_CONNECT_CANCEL (0x002B)`: Cancel connection attempt (ifdef CENTRAL)
- `REQ_GAP_SCAN_START (0x0030)`: Start scanning (ifdef CENTRAL)
- `REQ_GAP_SCAN_STOP (0x0031)`: Stop scanning (ifdef CENTRAL)
- `REQ_GATTC_MTU_REQUEST (0x00A0)`: Request MTU exchange (ifdef CENTRAL)
- `REQ_GATTC_SERVICE_DISCOVER (0x00A1)`: Discover services (ifdef CENTRAL)
- `REQ_GATTC_CHARACTERISTICS_DISCOVER (0x00A2)`: Discover characteristics (ifdef CENTRAL)
- `REQ_GATTC_DESCRIPTORS_DISCOVER (0x00A3)`: Discover descriptors (ifdef CENTRAL)
- `REQ_GATTC_READ (0x00A4)`: Read characteristic/descriptor (ifdef CENTRAL)
- `REQ_GATTC_WRITE (0x00A5)`: Write characteristic/descriptor (ifdef CENTRAL)

## Event Handling

### BLE Event Forwarding
All BLE events from the SoftDevice are captured and forwarded to the host:
- Connection events (with advertising data)
- Disconnection events
- MTU exchange requests
- Write requests
- Connection parameter updates
- PHY updates
- RSSI changes

### Event Processing Flow
1. SoftDevice generates event → `SD_EVT_IRQHandler` (Priority 4)
2. Event packaged into packet buffer
3. Special handling for specific events:
   - `BLE_GAP_EVT_CONNECTED`: Both advertising data and scan response data appended
   - `BLE_GAP_EVT_ADV_REPORT`: Advertisement data appended (Central mode only)
4. Event transmitted over TX SPI with code `RSP_EVENT_BLE`

### Interrupt Priorities
- **TX SPI (SPIM0)**: Highest application priority (Priority 2)
- **RX SPI (SPIS1)**: Lowest application priority (Priority 7)
- **SD Event Handler**: SoftDevice low priority (Priority 4)
- Reserved for SoftDevice: Priorities 0, 1, 4

## Memory Management

### Buffer Pools
- **TX Buffer Pool**: 8 buffers, each sized for max BLE event + 2 bytes
- **RX Buffer Pool**: 1 buffer for command reception
- **Packet Size**: `BLE_EVT_LEN_MAX(247) + 2` bytes
- **Advertising Buffers**: Double-buffered advertising/scan data (`BLE_GAP_ADV_SET_DATA_SIZE_MAX`)
- **Scan Buffers**: Two scan buffers for ping-pong operation (`BLE_GAP_SCAN_BUFFER_MAX` each)

### Queue Management
- TX packets queued with automatic transmission
- Hardware-triggered transmission via SPI interrupts
- Flow control through buffer pool exhaustion

### Advanced Buffer Strategies
- **Advertising Data Ping-Pong**: Uses alternating buffers to prevent corruption during updates
- **Scan Buffer Management**: Alternates between two scan buffers for continuous scanning
- **Static Allocation**: All buffers pre-allocated to avoid runtime heap fragmentation

## Key Observations

### What This Firmware DOES:
1. **Complete BLE Stack Access**: Provides full control over Nordic's S140 SoftDevice
2. **Dynamic Service Creation**: Host can create any GATT service structure at runtime
3. **Raw Event Access**: All BLE events forwarded with complete payloads
4. **Hardware Abstraction**: Shields host from Nordic-specific implementation details

### What This Firmware DOES NOT:
1. **No Predefined Services**: No built-in GATT services (Battery, DIS, etc.)
2. **No Application Logic**: Pure passthrough, no data processing
3. **No Security Implementation**: Security must be handled by host
4. **No Central Role**: Peripheral-only in current configuration

## Migration Implications for Rust Implementation

### Required Features for Compatibility:
1. **Dual SPI interfaces** with specific pin mappings
2. **Command-response protocol handler**
3. **Dynamic GATT service creation** based on host commands
4. **Complete BLE event forwarding**
5. **Buffer management** for async communication

### Architecture Decision Points:
1. **Maintain proxy architecture** vs implement services directly
2. **Keep dual SPI** vs switch to single SPI or UART
3. **Support dynamic services** vs fixed service structure
4. **Event filtering** vs complete event forwarding

### Recommended Approach:
Given the existing architecture, the host processor likely has significant BLE logic implemented. A phased migration approach would be:

1. **Phase 1**: Implement command-response protocol compatibility
2. **Phase 2**: Add fixed GATT services while maintaining proxy capability
3. **Phase 3**: Gradually migrate host logic into device firmware
4. **Phase 4**: Optimize communication protocol for reduced overhead

## Implementation Notes

### Main Loop Architecture
```c
for(;;) {
    sd_app_evt_wait();  // Sleep until SoftDevice event
}
```
The main loop uses Nordic's event-wait pattern for power efficiency.

### Special Features
- **Connection Event Extension**: Enabled for longer connection events
- **Data Length Extension**: Supported via `REQ_GAP_DATA_LENGTH_UPDATE`
- **2M PHY Support**: Available through `REQ_GAP_PHY_UPDATE`
- **Advertising Handles**: Support for multiple advertising sets

### Error Handling
- Assert/fault handler with debugging output
- System reset on critical errors
- Detailed error codes returned for all operations

### Memory Constraints
- Requires careful RAM allocation for SoftDevice
- Dynamic RAM calculation during initialization
- Static buffer pools to avoid heap fragmentation

## Custom UUID Base Reference
From your hardware documentation:
- Base UUID: `0x4340C3C8-9E75-47FB-B653-056FF74C6300`
- Service UUID offset: `0x0000`
- Characteristic UUID offset: `0x0001`

These would be registered via `REQ_REGISTER_UUID_GROUP` command in the current system.