//! Command Processing Module
//!
//! This module handles all BLE modem commands received from the host.
//! Commands are routed to appropriate handlers and responses are sent back.

use defmt::{debug, error, Format};
use heapless::Vec;
use nrf_softdevice::Softdevice;

use crate::core::memory::{BufferError, TxPacket};
use crate::core::protocol::serialization::*;
use crate::core::protocol::{Packet, ProtocolError, RequestCode, ResponseCode, MAX_PAYLOAD_SIZE};
use crate::core::transport;

pub mod gap;
pub mod gatts;
pub mod system;
pub mod uuid;

/// Command processing errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum CommandError {
    UnknownCommand,
    InvalidPayload,
    BufferError(BufferError),
    ProtocolError(ProtocolError),
    StateError(crate::ble::gatt_state::StateError),
    SoftDeviceError,
    NotImplemented,
}

impl From<BufferError> for CommandError {
    fn from(err: BufferError) -> Self {
        CommandError::BufferError(err)
    }
}

impl From<ProtocolError> for CommandError {
    fn from(err: ProtocolError) -> Self {
        CommandError::ProtocolError(err)
    }
}

impl From<crate::ble::gatt_state::StateError> for CommandError {
    fn from(err: crate::ble::gatt_state::StateError) -> Self {
        CommandError::StateError(err)
    }
}

/// Command response builder
pub struct ResponseBuilder {
    buffer: Vec<u8, MAX_PAYLOAD_SIZE>,
}

impl ResponseBuilder {
    /// Create a new response builder
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Add a u8 to the response
    pub fn add_u8(&mut self, value: u8) -> Result<&mut Self, CommandError> {
        write_u8(&mut self.buffer, value)?;
        Ok(self)
    }

    /// Add a u16 to the response
    pub fn add_u16(&mut self, value: u16) -> Result<&mut Self, CommandError> {
        write_u16(&mut self.buffer, value)?;
        Ok(self)
    }

    /// Add a u32 to the response
    pub fn add_u32(&mut self, value: u32) -> Result<&mut Self, CommandError> {
        write_u32(&mut self.buffer, value)?;
        Ok(self)
    }

    /// Add a byte slice to the response
    pub fn add_slice(&mut self, data: &[u8]) -> Result<&mut Self, CommandError> {
        write_slice(&mut self.buffer, data)?;
        Ok(self)
    }

    /// Add a string to the response
    pub fn add_string(&mut self, s: &str) -> Result<&mut Self, CommandError> {
        self.add_slice(s.as_bytes())
    }

    /// Build the response packet
    pub fn build(self, response_code: ResponseCode) -> Result<TxPacket, CommandError> {
        let packet = Packet::new_response(response_code, &self.buffer)?;
        let serialized = packet.serialize()?;
        let tx_packet = TxPacket::new(&serialized)?;
        Ok(tx_packet)
    }

    /// Build an ACK response with no payload
    pub fn build_ack() -> Result<TxPacket, CommandError> {
        Self::new().build(ResponseCode::Ack)
    }

    /// Build an error response with error code
    pub fn build_error_code(error_code: u16) -> Result<TxPacket, CommandError> {
        let mut builder = Self::new();
        builder.add_u16(error_code)?;
        builder.build(ResponseCode::Error)
    }

    /// Build an error response from CommandError
    pub fn build_error(error: CommandError) -> Result<TxPacket, CommandError> {
        let error_code = match error {
            CommandError::UnknownCommand => 0x01,
            CommandError::InvalidPayload => 0x02,
            CommandError::BufferError(_) => 0x03,
            CommandError::ProtocolError(_) => 0x04,
            CommandError::StateError(_) => 0x05,
            CommandError::SoftDeviceError => 0x06,
            CommandError::NotImplemented => 0x07,
        };
        Self::build_error_code(error_code)
    }
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Process a command packet and send response
pub async fn process_command(packet: Packet, sd: &Softdevice) -> Result<(), CommandError> {
    let request_code = packet.request_code().ok_or(CommandError::UnknownCommand)?;

    debug!("Processing command: {:?}", request_code);

    let response = match request_code {
        // System Commands
        RequestCode::GetInfo => system::handle_get_info(&packet.payload).await,
        RequestCode::Echo => system::handle_echo(&packet.payload).await,
        RequestCode::Shutdown => system::handle_shutdown(&packet.payload).await,
        RequestCode::Reboot => system::handle_reboot(&packet.payload).await,

        // UUID Management
        RequestCode::RegisterUuidGroup => uuid::handle_register_uuid_group(&packet.payload).await,

        // GAP Operations - Address Management
        RequestCode::GapGetAddr => gap::handle_get_addr(&packet.payload).await,
        RequestCode::GapSetAddr => gap::handle_set_addr(&packet.payload).await,

        // GAP Operations - Advertising Control
        RequestCode::GapAdvStart => gap::handle_adv_start(&packet.payload, sd).await,
        RequestCode::GapAdvStop => gap::handle_adv_stop(&packet.payload, sd).await,
        RequestCode::GapAdvSetConfigure => gap::handle_adv_configure(&packet.payload).await,

        // GAP Operations - Device Configuration
        RequestCode::GapGetName => gap::handle_get_name(&packet.payload).await,
        RequestCode::GapSetName => gap::handle_set_name(&packet.payload).await,
        RequestCode::GapConnParamsGet => gap::handle_conn_params_get(&packet.payload).await,
        RequestCode::GapConnParamsSet => gap::handle_conn_params_set(&packet.payload).await,

        // GAP Operations - Connection Management
        RequestCode::GapConnParamUpdate => gap::handle_conn_param_update(&packet.payload).await,
        RequestCode::GapDataLengthUpdate => gap::handle_data_length_update(&packet.payload).await,
        RequestCode::GapPhyUpdate => gap::handle_phy_update(&packet.payload).await,
        RequestCode::GapDisconnect => gap::handle_disconnect(&packet.payload).await,

        // GAP Operations - Power & RSSI
        RequestCode::GapSetTxPower => gap::handle_set_tx_power(&packet.payload).await,
        RequestCode::GapStartRssiReporting => gap::handle_start_rssi_reporting(&packet.payload).await,
        RequestCode::GapStopRssiReporting => gap::handle_stop_rssi_reporting(&packet.payload).await,

        // GATT Server Operations
        RequestCode::GattsServiceAdd => gatts::handle_service_add(&packet.payload, sd).await,
        RequestCode::GattsCharacteristicAdd => gatts::handle_characteristic_add(&packet.payload, sd).await,
        RequestCode::GattsMtuReply => gatts::handle_mtu_reply(&packet.payload).await,
        RequestCode::GattsHvx => gatts::handle_hvx(&packet.payload).await,
        RequestCode::GattsSysAttrGet => {
            error!("GattsSysAttrGet not implemented in original firmware");
            ResponseBuilder::build_error(CommandError::NotImplemented)
        }
        RequestCode::GattsSysAttrSet => gatts::handle_sys_attr_set(&packet.payload).await,

        // Central mode commands (not implemented in peripheral-only configuration)
        RequestCode::GapConnect
        | RequestCode::GapConnectCancel
        | RequestCode::GapScanStart
        | RequestCode::GapScanStop
        | RequestCode::GattcMtuRequest
        | RequestCode::GattcServiceDiscover
        | RequestCode::GattcCharacteristicsDiscover
        | RequestCode::GattcDescriptorsDiscover
        | RequestCode::GattcRead
        | RequestCode::GattcWrite => {
            debug!("Central mode command not supported: {:?}", request_code);
            ResponseBuilder::build_error(CommandError::NotImplemented)
        }
    };

    match response {
        Ok(tx_packet) => {
            debug!("Command processed successfully, sending response");
            transport::send_response(tx_packet)
                .await
                .map_err(|_| CommandError::BufferError(BufferError::PoolExhausted))?;
        }
        Err(e) => {
            error!("Command processing failed: {:?}", e);
            // Try to send error response
            if let Ok(error_packet) = ResponseBuilder::build_error(CommandError::UnknownCommand) {
                let _ = transport::send_response(error_packet).await;
            }
            return Err(e);
        }
    }

    Ok(())
}

/// Command processor state
pub struct CommandProcessor {
    // Future: Add command processing state/statistics here
}

impl CommandProcessor {
    /// Create a new command processor
    pub fn new() -> Self {
        Self {}
    }

    /// Process a single command
    pub async fn process_command(&mut self, packet: Packet, sd: &Softdevice) -> Result<TxPacket, CommandError> {
        let request_code = packet.request_code().ok_or(CommandError::UnknownCommand)?;

        debug!("Processing command: {:?}", request_code);

        match request_code {
            // System Commands
            RequestCode::GetInfo => system::handle_get_info(&packet.payload).await,
            RequestCode::Echo => system::handle_echo(&packet.payload).await,
            RequestCode::Shutdown => system::handle_shutdown(&packet.payload).await,
            RequestCode::Reboot => system::handle_reboot(&packet.payload).await,

            // UUID Management
            RequestCode::RegisterUuidGroup => uuid::handle_register_uuid_group(&packet.payload).await,

            // GAP Operations - Address Management
            RequestCode::GapGetAddr => gap::handle_get_addr(&packet.payload).await,
            RequestCode::GapSetAddr => gap::handle_set_addr(&packet.payload).await,

            // GAP Operations - Advertising Control
            RequestCode::GapAdvStart => gap::handle_adv_start(&packet.payload, sd).await,
            RequestCode::GapAdvStop => gap::handle_adv_stop(&packet.payload, sd).await,
            RequestCode::GapAdvSetConfigure => gap::handle_adv_configure(&packet.payload).await,

            // GAP Operations - Device Configuration
            RequestCode::GapGetName => gap::handle_get_name(&packet.payload).await,
            RequestCode::GapSetName => gap::handle_set_name(&packet.payload).await,
            RequestCode::GapConnParamsGet => gap::handle_conn_params_get(&packet.payload).await,
            RequestCode::GapConnParamsSet => gap::handle_conn_params_set(&packet.payload).await,

            // GAP Operations - Connection Management
            RequestCode::GapConnParamUpdate => gap::handle_conn_param_update(&packet.payload).await,
            RequestCode::GapDataLengthUpdate => gap::handle_data_length_update(&packet.payload).await,
            RequestCode::GapPhyUpdate => gap::handle_phy_update(&packet.payload).await,
            RequestCode::GapDisconnect => gap::handle_disconnect(&packet.payload).await,

            // GAP Operations - Power & RSSI
            RequestCode::GapSetTxPower => gap::handle_set_tx_power(&packet.payload).await,
            RequestCode::GapStartRssiReporting => gap::handle_start_rssi_reporting(&packet.payload).await,
            RequestCode::GapStopRssiReporting => gap::handle_stop_rssi_reporting(&packet.payload).await,

            // GATT Server Operations
            RequestCode::GattsServiceAdd => gatts::handle_service_add(&packet.payload, sd).await,
            RequestCode::GattsCharacteristicAdd => gatts::handle_characteristic_add(&packet.payload, sd).await,
            RequestCode::GattsMtuReply => gatts::handle_mtu_reply(&packet.payload).await,
            RequestCode::GattsHvx => gatts::handle_hvx(&packet.payload).await,
            RequestCode::GattsSysAttrGet => {
                error!("GattsSysAttrGet not implemented in original firmware");
                ResponseBuilder::build_error(CommandError::NotImplemented)
            }
            RequestCode::GattsSysAttrSet => gatts::handle_sys_attr_set(&packet.payload).await,

            // Central mode commands (not implemented in peripheral-only configuration)
            RequestCode::GapConnect
            | RequestCode::GapConnectCancel
            | RequestCode::GapScanStart
            | RequestCode::GapScanStop
            | RequestCode::GattcMtuRequest
            | RequestCode::GattcServiceDiscover
            | RequestCode::GattcCharacteristicsDiscover
            | RequestCode::GattcDescriptorsDiscover
            | RequestCode::GattcRead
            | RequestCode::GattcWrite => {
                debug!("Central mode command not supported: {:?}", request_code);
                ResponseBuilder::build_error(CommandError::NotImplemented)
            }
        }
    }
}

/// Main command processor task
#[embassy_executor::task]
pub async fn command_processor_task(sd: &'static Softdevice) {
    defmt::info!("Starting command processor task");

    loop {
        // Wait for command from SPI
        let packet = transport::receive_command().await;

        // Process the command
        if let Err(e) = process_command(packet, sd).await {
            error!("Command processing error: {:?}", e);
        }
    }
}
