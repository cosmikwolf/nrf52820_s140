//! BLE Modem Protocol Definitions
//! 
//! This module defines the communication protocol between the host and the BLE modem.
//! Protocol format:
//! - Request: [Payload Data] [Request Code (2 bytes, big-endian)]
//! - Response: [Response Code (2 bytes)] [Payload Data]

use defmt::Format;
use heapless::Vec;
use crc::{Crc, CRC_16_IBM_SDLC};

/// Maximum payload size (BLE_EVT_LEN_MAX + 2 bytes)
pub const MAX_PAYLOAD_SIZE: usize = 247 + 2;

/// CRC16-CCITT calculator for message validation
const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_SDLC);

/// Calculate CRC16 for a message
pub fn calculate_crc16(data: &[u8]) -> u16 {
    CRC16.checksum(data)
}

/// Validate CRC16 for a received message
pub fn validate_crc16(data: &[u8], expected_crc: u16) -> bool {
    let calculated_crc = calculate_crc16(data);
    calculated_crc == expected_crc
}

/// Request codes sent by host to device
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum RequestCode {
    // System Commands
    GetInfo = 0x0001,
    Echo = 0x0003,
    Shutdown = 0x0002,
    Reboot = 0x00F0,

    // UUID Management
    RegisterUuidGroup = 0x0010,

    // GAP Operations - Address Management
    GapGetAddr = 0x0011,
    GapSetAddr = 0x0012,

    // GAP Operations - Advertising Control
    GapAdvStart = 0x0020,
    GapAdvStop = 0x0021,
    GapAdvSetConfigure = 0x0022,

    // GAP Operations - Device Configuration
    GapGetName = 0x0023,
    GapSetName = 0x0024,
    GapConnParamsGet = 0x0025,
    GapConnParamsSet = 0x0026,

    // GAP Operations - Connection Management
    GapConnParamUpdate = 0x0027,
    GapDataLengthUpdate = 0x0028,
    GapPhyUpdate = 0x0029,
    GapConnect = 0x002A,           // Central mode only
    GapConnectCancel = 0x002B,     // Central mode only
    GapDisconnect = 0x002C,

    // GAP Operations - Power & RSSI
    GapSetTxPower = 0x002D,
    GapStartRssiReporting = 0x002E,
    GapStopRssiReporting = 0x002F,

    // GAP Operations - Scanning (Central mode only)
    GapScanStart = 0x0030,
    GapScanStop = 0x0031,

    // GATT Server Operations
    GattsServiceAdd = 0x0080,
    GattsCharacteristicAdd = 0x0081,
    GattsMtuReply = 0x0082,
    GattsHvx = 0x0083,
    GattsSysAttrGet = 0x0084,     // Not implemented in original
    GattsSysAttrSet = 0x0085,

    // GATT Client Operations (Central mode only)
    GattcMtuRequest = 0x00A0,
    GattcServiceDiscover = 0x00A1,
    GattcCharacteristicsDiscover = 0x00A2,
    GattcDescriptorsDiscover = 0x00A3,
    GattcRead = 0x00A4,
    GattcWrite = 0x00A5,
}

/// Response codes sent by device to host
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum ResponseCode {
    /// Command acknowledgment with results
    Ack = 0xAC50,
    /// Error response
    Error = 0xAC51,
    /// BLE event notification
    BleEvent = 0x8001,
    /// System-on-Chip event notification
    SocEvent = 0x8002,
}

/// Protocol packet structure
#[derive(Debug, Clone)]
pub struct Packet {
    pub code: u16,
    pub payload: Vec<u8, MAX_PAYLOAD_SIZE>,
}

/// Protocol error types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum ProtocolError {
    InvalidLength,
    InvalidCode,
    SerializationError,
    BufferFull,
    InvalidCrc,
    InvalidData,
}

impl RequestCode {
    /// Convert from raw u16 value
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0001 => Some(Self::GetInfo),
            0x0002 => Some(Self::Shutdown),
            0x0003 => Some(Self::Echo),
            0x00F0 => Some(Self::Reboot),
            0x0010 => Some(Self::RegisterUuidGroup),
            0x0011 => Some(Self::GapGetAddr),
            0x0012 => Some(Self::GapSetAddr),
            0x0020 => Some(Self::GapAdvStart),
            0x0021 => Some(Self::GapAdvStop),
            0x0022 => Some(Self::GapAdvSetConfigure),
            0x0023 => Some(Self::GapGetName),
            0x0024 => Some(Self::GapSetName),
            0x0025 => Some(Self::GapConnParamsGet),
            0x0026 => Some(Self::GapConnParamsSet),
            0x0027 => Some(Self::GapConnParamUpdate),
            0x0028 => Some(Self::GapDataLengthUpdate),
            0x0029 => Some(Self::GapPhyUpdate),
            0x002A => Some(Self::GapConnect),
            0x002B => Some(Self::GapConnectCancel),
            0x002C => Some(Self::GapDisconnect),
            0x002D => Some(Self::GapSetTxPower),
            0x002E => Some(Self::GapStartRssiReporting),
            0x002F => Some(Self::GapStopRssiReporting),
            0x0030 => Some(Self::GapScanStart),
            0x0031 => Some(Self::GapScanStop),
            0x0080 => Some(Self::GattsServiceAdd),
            0x0081 => Some(Self::GattsCharacteristicAdd),
            0x0082 => Some(Self::GattsMtuReply),
            0x0083 => Some(Self::GattsHvx),
            0x0084 => Some(Self::GattsSysAttrGet),
            0x0085 => Some(Self::GattsSysAttrSet),
            0x00A0 => Some(Self::GattcMtuRequest),
            0x00A1 => Some(Self::GattcServiceDiscover),
            0x00A2 => Some(Self::GattcCharacteristicsDiscover),
            0x00A3 => Some(Self::GattcDescriptorsDiscover),
            0x00A4 => Some(Self::GattcRead),
            0x00A5 => Some(Self::GattcWrite),
            _ => None,
        }
    }
}

impl ResponseCode {
    /// Convert to raw u16 value
    pub fn to_u16(self) -> u16 {
        self as u16
    }
}

impl Packet {
    /// Create a new request packet from received data
    /// Format: [Length:2][Payload:N][RequestCode:2][CRC16:2]
    pub fn new_request(data: &[u8]) -> Result<Self, ProtocolError> {
        if data.len() < 6 {  // Minimum: length(2) + code(2) + crc(2)
            return Err(ProtocolError::InvalidLength);
        }

        // Parse length header
        let length = u16::from_be_bytes([data[0], data[1]]) as usize;
        if length != data.len() {
            return Err(ProtocolError::InvalidLength);
        }

        // Extract CRC from last 2 bytes
        let crc_offset = data.len() - 2;
        let received_crc = u16::from_be_bytes([data[crc_offset], data[crc_offset + 1]]);
        
        // Validate CRC over entire message except CRC field
        let message_data = &data[..crc_offset];
        if !validate_crc16(message_data, received_crc) {
            return Err(ProtocolError::InvalidCrc);
        }

        // Extract request code (2 bytes before CRC)
        let code_offset = crc_offset - 2;
        let code = u16::from_be_bytes([data[code_offset], data[code_offset + 1]]);
        
        // Extract payload (everything between length and code)
        let mut packet_payload = Vec::new();
        packet_payload.extend_from_slice(&data[2..code_offset])
            .map_err(|_| ProtocolError::BufferFull)?;

        Ok(Self {
            code,
            payload: packet_payload,
        })
    }

    /// Create a new response packet (code comes first, then payload)
    pub fn new_response(code: ResponseCode, payload: &[u8]) -> Result<Self, ProtocolError> {
        let mut packet_payload = Vec::new();
        packet_payload.extend_from_slice(payload)
            .map_err(|_| ProtocolError::BufferFull)?;

        Ok(Self {
            code: code.to_u16(),
            payload: packet_payload,
        })
    }
    
    /// Create a new request packet for sending (used in tests)
    /// This creates the packet structure, call serialize() to get bytes for transmission
    pub fn new_request_for_sending(code: RequestCode, payload: &[u8]) -> Result<Self, ProtocolError> {
        let mut packet_payload = Vec::new();
        packet_payload.extend_from_slice(payload)
            .map_err(|_| ProtocolError::BufferFull)?;

        Ok(Self {
            code: code as u16,
            payload: packet_payload,
        })
    }
    
    /// Serialize request packet to bytes for transmission
    /// Format: [Length:2][Payload:N][RequestCode:2][CRC16:2]
    pub fn serialize_request(&self) -> Result<Vec<u8, MAX_PAYLOAD_SIZE>, ProtocolError> {
        let mut message = Vec::new();
        
        // Calculate total length (length header + payload + code + crc)
        let total_length = 2 + self.payload.len() + 2 + 2;
        if total_length > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::BufferFull);
        }
        
        // Add length header
        let length_bytes = (total_length as u16).to_be_bytes();
        message.extend_from_slice(&length_bytes)
            .map_err(|_| ProtocolError::BufferFull)?;
            
        // Add payload
        message.extend_from_slice(&self.payload)
            .map_err(|_| ProtocolError::BufferFull)?;
        
        // Add request code
        let code_bytes = self.code.to_be_bytes();
        message.extend_from_slice(&code_bytes)
            .map_err(|_| ProtocolError::BufferFull)?;
        
        // Calculate and add CRC over everything except CRC itself
        let crc = calculate_crc16(&message);
        let crc_bytes = crc.to_be_bytes();
        message.extend_from_slice(&crc_bytes)
            .map_err(|_| ProtocolError::BufferFull)?;

        Ok(message)
    }

    /// Serialize packet to bytes for transmission
    /// Format: [Length:2][ResponseCode:2][Payload:N][CRC16:2]
    pub fn serialize(&self) -> Result<Vec<u8, MAX_PAYLOAD_SIZE>, ProtocolError> {
        let mut message = Vec::new();
        
        // Calculate total length (length header + code + payload + crc)
        let total_length = 2 + 2 + self.payload.len() + 2;
        if total_length > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::BufferFull);
        }
        
        // Add length header
        let length_bytes = (total_length as u16).to_be_bytes();
        message.extend_from_slice(&length_bytes)
            .map_err(|_| ProtocolError::BufferFull)?;
        
        // Add response code
        let code_bytes = self.code.to_be_bytes();
        message.extend_from_slice(&code_bytes)
            .map_err(|_| ProtocolError::BufferFull)?;
            
        // Add payload
        message.extend_from_slice(&self.payload)
            .map_err(|_| ProtocolError::BufferFull)?;
        
        // Calculate and add CRC over everything except CRC itself
        let crc = calculate_crc16(&message);
        let crc_bytes = crc.to_be_bytes();
        message.extend_from_slice(&crc_bytes)
            .map_err(|_| ProtocolError::BufferFull)?;

        Ok(message)
    }

    /// Get the request code (if this is a request packet)
    pub fn request_code(&self) -> Option<RequestCode> {
        RequestCode::from_u16(self.code)
    }

    /// Get the response code (if this is a response packet)
    pub fn response_code(&self) -> Option<ResponseCode> {
        match self.code {
            0xAC50 => Some(ResponseCode::Ack),
            0x8001 => Some(ResponseCode::BleEvent),
            0x8002 => Some(ResponseCode::SocEvent),
            _ => None,
        }
    }
}

/// Helper functions for big-endian serialization
pub mod serialization {
    use super::ProtocolError;
    use heapless::Vec;

    pub fn write_u8<const N: usize>(buffer: &mut Vec<u8, N>, value: u8) -> Result<(), ProtocolError> {
        buffer.push(value).map_err(|_| ProtocolError::BufferFull)
    }

    pub fn write_u16<const N: usize>(buffer: &mut Vec<u8, N>, value: u16) -> Result<(), ProtocolError> {
        let bytes = value.to_be_bytes();
        buffer.extend_from_slice(&bytes).map_err(|_| ProtocolError::BufferFull)
    }

    pub fn write_u32<const N: usize>(buffer: &mut Vec<u8, N>, value: u32) -> Result<(), ProtocolError> {
        let bytes = value.to_be_bytes();
        buffer.extend_from_slice(&bytes).map_err(|_| ProtocolError::BufferFull)
    }

    pub fn write_slice<const N: usize>(buffer: &mut Vec<u8, N>, data: &[u8]) -> Result<(), ProtocolError> {
        buffer.extend_from_slice(data).map_err(|_| ProtocolError::BufferFull)
    }

    pub fn read_u8(data: &[u8], offset: usize) -> Option<u8> {
        data.get(offset).copied()
    }

    pub fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
        if data.len() < offset + 2 {
            return None;
        }
        Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
    }

    pub fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
        if data.len() < offset + 4 {
            return None;
        }
        Some(u32::from_be_bytes([
            data[offset], 
            data[offset + 1], 
            data[offset + 2], 
            data[offset + 3]
        ]))
    }

    /// Helper for reading from payload sequentially
    pub struct PayloadReader<'a> {
        data: &'a [u8],
        offset: usize,
    }

    impl<'a> PayloadReader<'a> {
        pub fn new(data: &'a [u8]) -> Self {
            Self { data, offset: 0 }
        }

        pub fn read_u8(&mut self) -> Result<u8, ProtocolError> {
            if self.offset >= self.data.len() {
                return Err(ProtocolError::InvalidData);
            }
            let value = self.data[self.offset];
            self.offset += 1;
            Ok(value)
        }

        pub fn read_u16(&mut self) -> Result<u16, ProtocolError> {
            if self.offset + 2 > self.data.len() {
                return Err(ProtocolError::InvalidData);
            }
            let value = u16::from_be_bytes([
                self.data[self.offset],
                self.data[self.offset + 1]
            ]);
            self.offset += 2;
            Ok(value)
        }

        pub fn read_u32(&mut self) -> Result<u32, ProtocolError> {
            if self.offset + 4 > self.data.len() {
                return Err(ProtocolError::InvalidData);
            }
            let value = u32::from_be_bytes([
                self.data[self.offset],
                self.data[self.offset + 1],
                self.data[self.offset + 2],
                self.data[self.offset + 3]
            ]);
            self.offset += 4;
            Ok(value)
        }

        pub fn read_slice(&mut self, len: usize) -> Result<&'a [u8], ProtocolError> {
            if self.offset + len > self.data.len() {
                return Err(ProtocolError::InvalidData);
            }
            let slice = &self.data[self.offset..self.offset + len];
            self.offset += len;
            Ok(slice)
        }

        pub fn offset(&self) -> usize {
            self.offset
        }

        pub fn remaining(&self) -> usize {
            self.data.len() - self.offset
        }
    }
}