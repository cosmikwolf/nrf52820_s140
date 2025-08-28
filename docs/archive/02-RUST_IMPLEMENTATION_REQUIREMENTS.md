# Rust BLE Modem Implementation Requirements

## Core Architecture
The firmware acts as a **BLE proxy/passthrough device** that bridges a host processor to the nRF52 BLE stack via dual SPI channels. It does NOT implement GATT services directly - all BLE operations are controlled by the host through commands.

## Essential Hardware Configuration

### Dual SPI Interfaces (REQUIRED)
**TX SPI (SPIM0 - Master)**: Device → Host
- SS: Pin 1, SCK: Pin 0, MOSI: Pin 4
- 8 MHz, CPOL=High, CPHA=Leading, MSB First

**RX SPI (SPIS1 - Slave)**: Host → Device  
- SS: Pin 7 (pull-up), SCK: Pin 6, MOSI: Pin 5
- CPOL=High, CPHA=Leading, MSB First

### Power Configuration
- DC/DC converter enabled
- Constant latency mode for predictable timing
- Synthesized clock (20 PPM accuracy)

## Communication Protocol (REQUIRED)

### Message Formats
**Request**: `[Payload Data] [Request Code (2 bytes, BE)]`
**Response**: `[Response Code (2 bytes)] [Payload Data]`

### Response Codes
- `0xAC50`: ACK with results
- `0x8001`: BLE event notification
- `0x8002`: SOC event notification

## BLE Stack Configuration

### Required Parameters
- Role: Peripheral only
- Max connections: 1
- MTU: 247 bytes
- GAP Event Length: 30ms (24 * 1.25ms)
- GATTS Attribute Table: 1408 bytes
- VS UUID Count: 4
- Service Changed: Enabled

## Required Command Set

### System Commands
- `0x0001`: Get firmware version
- `0x0002`: Shutdown system
- `0x00F0`: System reboot

### UUID Management
- `0x0010`: Register 128-bit vendor UUID base

### GAP Operations (Essential)
- `0x0011/0x0012`: Get/Set BLE address
- `0x0020/0x0021`: Start/Stop advertising
- `0x0022`: Configure advertising
- `0x0023/0x0024`: Get/Set device name
- `0x0025/0x0026`: Get/Set connection parameters
- `0x0027`: Update connection parameters
- `0x0028`: Update data length extension
- `0x0029`: Update PHY (1M/2M/Coded)
- `0x002C`: Disconnect
- `0x002D`: Set TX power
- `0x002E/0x002F`: Start/Stop RSSI reporting

### GATTS Operations (Essential)
- `0x0080`: Add service (Primary/Secondary)
- `0x0081`: Add characteristic with full metadata:
  - Properties, descriptors, permissions
  - CCCD/SCCD support
  - Variable length attributes
- `0x0082`: MTU exchange reply
- `0x0083`: Send notification/indication (with blocking option)
- `0x0084`: Get system attributes (NOT IMPLEMENTED in original)
- `0x0085`: Set system attributes (bonding)

## Event Forwarding Requirements

### Must Forward All BLE Events
- Connection events (include advertising data)
- Disconnection events
- MTU exchange requests
- Write requests
- Connection parameter updates
- PHY updates
- RSSI changes

### Event Processing
1. Capture SoftDevice events
2. Package with appropriate response code
3. Append event-specific data
4. Transmit via TX SPI

## Memory Management

### Buffer Requirements
- **TX Pool**: 8 buffers of `BLE_EVT_LEN_MAX(247) + 2` bytes
- **RX Pool**: 1 command reception buffer
- **Advertising**: Double-buffered for ping-pong updates
- **Static Allocation**: Pre-allocate all buffers

### Interrupt Priorities
- TX SPI: Priority 2
- RX SPI: Priority 7
- SD Events: Priority 4
- Reserved for SoftDevice: 0, 1, 4

## Implementation Priority

### Phase 1: Core Proxy (REQUIRED for drop-in)
1. Dual SPI communication
2. Command-response protocol handler
3. BLE stack initialization with exact parameters
4. GAP advertising commands
5. Basic GATTS service creation
6. Event forwarding system

### Phase 2: Full Compatibility
1. Complete command set implementation
2. Connection parameter management
3. RSSI reporting
4. System attribute handling
5. PHY and data length updates

### Phase 3: Optional Enhancements
1. Central role support (currently disabled)
2. Multiple connection support
3. Protocol optimizations

## Critical Success Factors
- **Exact protocol compatibility**: Message formats must match precisely
- **Complete event forwarding**: Host expects all BLE events
- **Dynamic service creation**: Host controls all GATT structure
- **Buffer management**: Must handle async communication reliably
- **Pin compatibility**: Exact SPI pin mappings required

## Testing Requirements
1. Verify SPI communication with existing host
2. Test all command codes return expected responses
3. Validate event forwarding includes all data
4. Confirm GATT service creation from host commands
5. Test advertising and connection establishment
6. Verify MTU exchange and data transfer

## Known UUID Base
- Base: `0x4340C3C8-9E75-47FB-B653-056FF74C6300`
- Service offset: `0x0000`
- Characteristic offset: `0x0001`