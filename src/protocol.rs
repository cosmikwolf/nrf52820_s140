//! BLE Modem Protocol Definitions
//! 
//! This module defines the communication protocol between the host and the BLE modem.
//! Protocol format:
//! - Request: [Payload Data] [Request Code (2 bytes, big-endian)]
//! - Response: [Response Code (2 bytes)] [Payload Data]

use defmt::Format;
use heapless::Vec;

/// Maximum payload size (BLE_EVT_LEN_MAX + 2 bytes)
pub const MAX_PAYLOAD_SIZE: usize = 247 + 2;

/// Request codes sent by host to device
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum RequestCode {
    // System Commands
    GetInfo = 0x0001,
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
    /// BLE event notification
    BleEvent = 0x8001,
    /// System-on-Chip event notification
    SocEvent = 0x8002,
}

/// Protocol packet structure
#[derive(Debug, Clone, Format)]
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
}

impl RequestCode {
    /// Convert from raw u16 value
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0001 => Some(Self::GetInfo),
            0x0002 => Some(Self::Shutdown),
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
    /// Create a new request packet (payload comes first, then code)
    pub fn new_request(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 2 {
            return Err(ProtocolError::InvalidLength);
        }

        let len = payload.len();
        let code_bytes = &payload[len-2..];
        let code = u16::from_be_bytes([code_bytes[0], code_bytes[1]]);
        
        let mut packet_payload = Vec::new();
        packet_payload.extend_from_slice(&payload[..len-2])
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

    /// Serialize packet to bytes for transmission
    pub fn serialize(&self) -> Result<Vec<u8, MAX_PAYLOAD_SIZE>, ProtocolError> {
        let mut buffer = Vec::new();
        
        // For response: code first, then payload
        let code_bytes = self.code.to_be_bytes();
        buffer.extend_from_slice(&code_bytes)
            .map_err(|_| ProtocolError::BufferFull)?;
        buffer.extend_from_slice(&self.payload)
            .map_err(|_| ProtocolError::BufferFull)?;

        Ok(buffer)
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
}