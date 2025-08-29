//! GAP (Generic Access Profile) Commands Implementation
//!
//! Handles GAP operations including advertising, connection management,
//! device configuration, and power management.

use defmt::{debug, error, info};
use nrf_softdevice::ble::{Address, AddressType};
use nrf_softdevice::Softdevice;

use crate::ble::{advertising, gap_state};
use crate::commands::{CommandError, ResponseBuilder};
use crate::core::memory::TxPacket;
use crate::core::protocol::serialization::PayloadReader;

// Placeholder implementations - will be completed in later phases

pub async fn handle_get_addr(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: GET_ADDR");

    // Get device address from SoftDevice using nrf-softdevice wrapper
    let addr = nrf_softdevice::ble::get_address(unsafe { &nrf_softdevice::Softdevice::steal() });

    let mut response = ResponseBuilder::new();
    response.add_u32(nrf_softdevice::raw::NRF_SUCCESS)?; // Always successful with wrapper

    // Add address type and 6-byte address
    let addr_type = (addr.flags >> 1) & 0x7F; // Extract address type from flags
    response.add_u8(addr_type)?;
    response.add_slice(&addr.bytes)?;

    // Update our internal state
    let mut state = gap_state::gap_state().lock().await;
    state.device_addr.copy_from_slice(&addr.bytes);
    state.addr_type = addr_type;

    response.build(crate::core::protocol::ResponseCode::Ack)
}

pub async fn handle_set_addr(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: SET_ADDR");

    if payload.len() < 7 {
        // addr_type (1) + addr (6)
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    let addr_type_u8 = reader.read_u8()?;
    let addr_bytes = reader.read_slice(6)?;

    // Convert to AddressType enum
    let addr_type = match addr_type_u8 {
        0 => AddressType::Public,
        1 => AddressType::RandomStatic,
        2 => AddressType::RandomPrivateResolvable,
        3 => AddressType::RandomPrivateNonResolvable,
        _ => return ResponseBuilder::build_error(CommandError::InvalidPayload),
    };

    let mut addr_array = [0u8; 6];
    addr_array.copy_from_slice(addr_bytes);
    let addr = Address::new(addr_type, addr_array);

    // Set address using nrf-softdevice wrapper
    nrf_softdevice::ble::set_address(unsafe { &nrf_softdevice::Softdevice::steal() }, &addr);

    // Update our internal state
    let mut state = gap_state::gap_state().lock().await;
    state.device_addr.copy_from_slice(&addr.bytes);
    state.addr_type = addr_type_u8;

    let mut response = ResponseBuilder::new();
    response.add_u32(nrf_softdevice::raw::NRF_SUCCESS)?;
    response.build(crate::core::protocol::ResponseCode::Ack)
}

pub async fn handle_adv_start(payload: &[u8], _sd: &Softdevice) -> Result<TxPacket, CommandError> {
    debug!("GAP: ADV_START");

    if payload.len() < 2 {
        // adv_handle + conn_cfg_tag
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    let adv_handle = reader.read_u8()?;
    let conn_cfg_tag = reader.read_u8()?;

    // Send command to advertising controller
    let cmd = advertising::AdvCommand::Start {
        handle: adv_handle,
        conn_cfg_tag,
    };

    let result = if advertising::send_command(cmd).is_ok() {
        nrf_softdevice::raw::NRF_SUCCESS
    } else {
        nrf_softdevice::raw::NRF_ERROR_NO_MEM // Command queue full
    };

    let mut response = ResponseBuilder::new();
    response.add_u32(result)?;
    response.build(crate::core::protocol::ResponseCode::Ack)
}

pub async fn handle_adv_stop(payload: &[u8], _sd: &Softdevice) -> Result<TxPacket, CommandError> {
    debug!("GAP: ADV_STOP");

    if payload.len() < 1 {
        // adv_handle
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    let adv_handle = reader.read_u8()?;

    // Send command to advertising controller
    let cmd = advertising::AdvCommand::Stop { handle: adv_handle };

    let result = if advertising::send_command(cmd).is_ok() {
        nrf_softdevice::raw::NRF_SUCCESS
    } else {
        nrf_softdevice::raw::NRF_ERROR_NO_MEM // Command queue full
    };

    let mut response = ResponseBuilder::new();
    response.add_u32(result)?;
    response.build(crate::core::protocol::ResponseCode::Ack)
}

pub async fn handle_adv_configure(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: ADV_CONFIGURE");

    if payload.len() < 2 {
        // At least handle + data present flag
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    let handle = reader.read_u8()?;
    let data_present = reader.read_u8()? != 0;

    // TODO: Parse advertising parameters and data from payload
    // For now, we'll use a simplified approach that works with the controller

    if data_present && reader.remaining() >= 4 {
        // Read advertising data length and scan response length
        let adv_data_len = reader.read_u16()? as usize;
        let scan_rsp_len = reader.read_u16()? as usize;

        // Validate lengths
        if adv_data_len <= 31 && scan_rsp_len <= 31 && reader.remaining() >= adv_data_len + scan_rsp_len {
            let adv_data = reader.read_slice(adv_data_len)?;
            let scan_data = reader.read_slice(scan_rsp_len)?;

            // Store in gap state for the advertising controller to use
            {
                let mut state = gap_state::gap_state().lock().await;
                state.set_adv_data(adv_data);
                state.set_scan_response(scan_data);
                state.adv_handle = handle;
            }

            debug!(
                "Configured advertising data: {} bytes adv, {} bytes scan",
                adv_data_len, scan_rsp_len
            );
        }
    }

    // Send configure command to advertising controller
    let cmd = advertising::AdvCommand::Configure { handle, data_present };
    let result = if advertising::send_command(cmd).is_ok() {
        nrf_softdevice::raw::NRF_SUCCESS
    } else {
        nrf_softdevice::raw::NRF_ERROR_NO_MEM
    };

    let mut response = ResponseBuilder::new();
    response.add_u32(result)?;
    response.add_u8(handle)?;
    response.build(crate::core::protocol::ResponseCode::Ack)
}

pub async fn handle_get_name(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: GET_NAME");

    if payload.len() < 1 {
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    let present = reader.read_u8()? != 0;

    let mut response = ResponseBuilder::new();
    response.add_u32(nrf_softdevice::raw::NRF_SUCCESS)?;

    if present {
        // Get device name from our internal state
        let state = gap_state::gap_state().lock().await;
        let device_name = state.device_name();

        // Add length as u16, then the name data
        response.add_u16(device_name.len() as u16)?;
        response.add_slice(device_name)?;
    } else {
        // Just return the length
        let state = gap_state::gap_state().lock().await;
        response.add_u16(state.device_name_len as u16)?;
    }

    response.build(crate::core::protocol::ResponseCode::Ack)
}

pub async fn handle_set_name(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: SET_NAME");

    if payload.len() < 2 {
        // At least security mode (2 bytes)
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    let sec_mode_sm = reader.read_u8()?; // Security mode sm field
    let sec_mode_lv = reader.read_u8()?; // Security mode lv field

    // Remaining payload is the device name
    let name_bytes = &payload[2..];

    // Set device name using SoftDevice API
    let sec_mode = nrf_softdevice::raw::ble_gap_conn_sec_mode_t {
        _bitfield_1: nrf_softdevice::raw::ble_gap_conn_sec_mode_t::new_bitfield_1(sec_mode_sm, sec_mode_lv),
    };

    let result = unsafe {
        nrf_softdevice::raw::sd_ble_gap_device_name_set(
            &sec_mode as *const _,
            name_bytes.as_ptr(),
            name_bytes.len() as u16,
        )
    };

    // Update our internal state if successful
    if result == nrf_softdevice::raw::NRF_SUCCESS {
        let mut state = gap_state::gap_state().lock().await;
        state.set_device_name(name_bytes);
    }

    let mut response = ResponseBuilder::new();
    response.add_u32(result)?;
    response.build(crate::core::protocol::ResponseCode::Ack)
}

pub async fn handle_conn_params_get(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: CONN_PARAMS_GET");

    // Get PPCP (Peripheral Preferred Connection Parameters) from SoftDevice
    let mut conn_params = nrf_softdevice::raw::ble_gap_conn_params_t {
        min_conn_interval: 0,
        max_conn_interval: 0,
        slave_latency: 0,
        conn_sup_timeout: 0,
    };

    let result = unsafe { nrf_softdevice::raw::sd_ble_gap_ppcp_get(&mut conn_params as *mut _) };

    let mut response = ResponseBuilder::new();
    response.add_u32(result)?;

    if result == nrf_softdevice::raw::NRF_SUCCESS {
        // Add connection parameters (4 x u16 = 8 bytes)
        response.add_u16(conn_params.min_conn_interval)?;
        response.add_u16(conn_params.max_conn_interval)?;
        response.add_u16(conn_params.slave_latency)?;
        response.add_u16(conn_params.conn_sup_timeout)?;

        // Update our internal state
        let mut state = gap_state::gap_state().lock().await;
        state.preferred_conn_params.min_conn_interval = conn_params.min_conn_interval;
        state.preferred_conn_params.max_conn_interval = conn_params.max_conn_interval;
        state.preferred_conn_params.slave_latency = conn_params.slave_latency;
        state.preferred_conn_params.conn_sup_timeout = conn_params.conn_sup_timeout;
    }

    response.build(crate::core::protocol::ResponseCode::Ack)
}

pub async fn handle_conn_params_set(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: CONN_PARAMS_SET");

    if payload.len() < 8 {
        // 4 x u16 = 8 bytes
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let mut reader = PayloadReader::new(payload);
    let min_conn_interval = reader.read_u16()?;
    let max_conn_interval = reader.read_u16()?;
    let slave_latency = reader.read_u16()?;
    let conn_sup_timeout = reader.read_u16()?;

    let conn_params = nrf_softdevice::raw::ble_gap_conn_params_t {
        min_conn_interval,
        max_conn_interval,
        slave_latency,
        conn_sup_timeout,
    };

    let result = unsafe { nrf_softdevice::raw::sd_ble_gap_ppcp_set(&conn_params as *const _) };

    // Update our internal state if successful
    if result == nrf_softdevice::raw::NRF_SUCCESS {
        let mut state = gap_state::gap_state().lock().await;
        state.preferred_conn_params = crate::ble::gap_state::ConnectionParams {
            min_conn_interval,
            max_conn_interval,
            slave_latency,
            conn_sup_timeout,
        };
    }

    let mut response = ResponseBuilder::new();
    response.add_u32(result)?;
    response.build(crate::core::protocol::ResponseCode::Ack)
}

/// Handle CONNECTION_PARAM_UPDATE command (0x0027)
/// Updates connection parameters for an active connection
///
/// Payload format:
/// - 2 bytes: Connection handle
/// - 2 bytes: Min connection interval (1.25ms units)
/// - 2 bytes: Max connection interval (1.25ms units) 
/// - 2 bytes: Slave latency
/// - 2 bytes: Connection supervision timeout (10ms units)
pub async fn handle_conn_param_update(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: CONN_PARAM_UPDATE requested");

    if payload.len() != 10 {
        debug!("GAP: Invalid payload length: {} (expected 10)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    // Parse connection parameters
    let conn_handle = u16::from_le_bytes([payload[0], payload[1]]);
    let min_conn_interval = u16::from_le_bytes([payload[2], payload[3]]);
    let max_conn_interval = u16::from_le_bytes([payload[4], payload[5]]);
    let slave_latency = u16::from_le_bytes([payload[6], payload[7]]);
    let conn_sup_timeout = u16::from_le_bytes([payload[8], payload[9]]);

    debug!("GAP: Updating connection parameters for handle {}: min={}, max={}, latency={}, timeout={}", 
           conn_handle, min_conn_interval, max_conn_interval, slave_latency, conn_sup_timeout);

    // Create connection parameters structure
    let conn_params = nrf_softdevice::raw::ble_gap_conn_params_t {
        min_conn_interval,
        max_conn_interval,
        slave_latency,
        conn_sup_timeout,
    };

    // Call SoftDevice API to update connection parameters
    let ret = unsafe {
        nrf_softdevice::raw::sd_ble_gap_conn_param_update(
            conn_handle,
            &conn_params as *const nrf_softdevice::raw::ble_gap_conn_params_t,
        )
    };

    if ret != nrf_softdevice::raw::NRF_SUCCESS {
        error!("GAP: Failed to update connection parameters: error code {}", ret);
        return ResponseBuilder::build_error(CommandError::SoftDeviceError);
    }

    info!("GAP: Connection parameter update initiated successfully");
    ResponseBuilder::build_ack()
}

/// Handle DATA_LENGTH_UPDATE command (0x0028)
/// Requests to update the data length parameters for a connection
///
/// Payload format:
/// - 2 bytes: Connection handle
/// - 2 bytes: TX octets (maximum number of payload octets to send)
/// - 2 bytes: TX time (maximum time in microseconds for TX)
pub async fn handle_data_length_update(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: DATA_LENGTH_UPDATE requested");

    if payload.len() != 6 {
        debug!("GAP: Invalid payload length: {} (expected 6)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let conn_handle = u16::from_le_bytes([payload[0], payload[1]]);
    let tx_octets = u16::from_le_bytes([payload[2], payload[3]]);
    let tx_time_us = u16::from_le_bytes([payload[4], payload[5]]);

    debug!("GAP: Updating data length for handle {}: tx_octets={}, tx_time={}us", 
           conn_handle, tx_octets, tx_time_us);

    // Create data length parameters structure
    let dl_params = nrf_softdevice::raw::ble_gap_data_length_params_t {
        max_tx_octets: tx_octets,
        max_rx_octets: 0, // Let SoftDevice choose
        max_tx_time_us: tx_time_us,
        max_rx_time_us: 0, // Let SoftDevice choose
    };

    let mut dl_limitation = nrf_softdevice::raw::ble_gap_data_length_limitation_t {
        tx_payload_limited_octets: 0,
        rx_payload_limited_octets: 0,
        tx_rx_time_limited_us: 0,
    };

    // Call SoftDevice API to request data length update
    let ret = unsafe {
        nrf_softdevice::raw::sd_ble_gap_data_length_update(
            conn_handle,
            &dl_params as *const nrf_softdevice::raw::ble_gap_data_length_params_t,
            &mut dl_limitation as *mut nrf_softdevice::raw::ble_gap_data_length_limitation_t,
        )
    };

    if ret != nrf_softdevice::raw::NRF_SUCCESS {
        error!("GAP: Failed to update data length: error code {}", ret);
        return ResponseBuilder::build_error(CommandError::SoftDeviceError);
    }

    info!("GAP: Data length update initiated successfully");
    ResponseBuilder::build_ack()
}

/// Handle PHY_UPDATE command (0x0029)
/// Requests to update the PHY preferences for a connection
///
/// Payload format:
/// - 2 bytes: Connection handle
/// - 1 byte: TX PHYs preference (bitmask: 0x01=1M, 0x02=2M, 0x04=Coded)
/// - 1 byte: RX PHYs preference (bitmask: 0x01=1M, 0x02=2M, 0x04=Coded)
/// - 2 bytes: Coded PHY preference (0x0000=No preference, 0x0001=S2, 0x0002=S8)
pub async fn handle_phy_update(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: PHY_UPDATE requested");

    if payload.len() != 6 {
        debug!("GAP: Invalid payload length: {} (expected 6)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let conn_handle = u16::from_le_bytes([payload[0], payload[1]]);
    let tx_phys = payload[2];
    let rx_phys = payload[3];
    let _phy_options = u16::from_le_bytes([payload[4], payload[5]]); // Not used by SoftDevice API

    debug!("GAP: Updating PHY for handle {}: tx_phys=0x{:02X}, rx_phys=0x{:02X}", 
           conn_handle, tx_phys, rx_phys);

    // Create PHY parameters structure
    let phy_params = nrf_softdevice::raw::ble_gap_phys_t {
        tx_phys,
        rx_phys,
    };

    // Call SoftDevice API to update PHY
    let ret = unsafe {
        nrf_softdevice::raw::sd_ble_gap_phy_update(
            conn_handle,
            &phy_params as *const nrf_softdevice::raw::ble_gap_phys_t,
        )
    };

    if ret != nrf_softdevice::raw::NRF_SUCCESS {
        error!("GAP: Failed to update PHY: error code {}", ret);
        return ResponseBuilder::build_error(CommandError::SoftDeviceError);
    }

    info!("GAP: PHY update initiated successfully");
    ResponseBuilder::build_ack()
}

/// Handle DISCONNECT command (0x002C)
/// Terminates an active BLE connection
///
/// Payload format:
/// - 2 bytes: Connection handle
/// - 1 byte: HCI disconnect reason code
pub async fn handle_disconnect(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: DISCONNECT requested");

    if payload.len() != 3 {
        debug!("GAP: Invalid payload length: {} (expected 3)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let conn_handle = u16::from_le_bytes([payload[0], payload[1]]);
    let reason = payload[2];

    debug!("GAP: Disconnecting connection handle {} with reason {}", conn_handle, reason);

    // Call SoftDevice API to disconnect
    let ret = unsafe {
        nrf_softdevice::raw::sd_ble_gap_disconnect(conn_handle, reason)
    };

    if ret != nrf_softdevice::raw::NRF_SUCCESS {
        error!("GAP: Failed to disconnect: error code {}", ret);
        return ResponseBuilder::build_error(CommandError::SoftDeviceError);
    }

    info!("GAP: Connection disconnect initiated successfully");
    ResponseBuilder::build_ack()
}

/// Handle SET_TX_POWER command (0x002D)
/// Sets the transmit power level
///
/// Payload format:
/// - 1 byte: Role (0x01=Advertising, 0x02=Scanning, 0x03=Connection)
/// - 2 bytes: Connection handle (only for connection role, otherwise ignored)
/// - 1 byte: TX power level in dBm (-40 to +8 dBm)
pub async fn handle_set_tx_power(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: SET_TX_POWER requested");

    if payload.len() != 4 {
        debug!("GAP: Invalid payload length: {} (expected 4)", payload.len());
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }

    let role = payload[0];
    let conn_handle = u16::from_le_bytes([payload[1], payload[2]]);
    let tx_power = payload[3] as i8; // Convert to signed

    debug!("GAP: Setting TX power for role {} handle {}: {}dBm", role, conn_handle, tx_power);

    // Call SoftDevice API to set TX power
    let ret = unsafe {
        match role {
            0x01 => {
                // Advertising TX power
                nrf_softdevice::raw::sd_ble_gap_tx_power_set(
                    1, // BLE_GAP_TX_POWER_ROLE_ADV
                    0, // Handle not used for advertising
                    tx_power,
                )
            }
            0x02 => {
                // Scanning TX power (not typically used but supported)
                nrf_softdevice::raw::sd_ble_gap_tx_power_set(
                    2, // BLE_GAP_TX_POWER_ROLE_SCAN_INIT
                    0, // Handle not used for scanning
                    tx_power,
                )
            }
            0x03 => {
                // Connection TX power
                nrf_softdevice::raw::sd_ble_gap_tx_power_set(
                    3, // BLE_GAP_TX_POWER_ROLE_CONN
                    conn_handle,
                    tx_power,
                )
            }
            _ => {
                error!("GAP: Invalid TX power role: {}", role);
                return ResponseBuilder::build_error(CommandError::InvalidPayload);
            }
        }
    };

    if ret != nrf_softdevice::raw::NRF_SUCCESS {
        error!("GAP: Failed to set TX power: error code {}", ret);
        return ResponseBuilder::build_error(CommandError::SoftDeviceError);
    }

    info!("GAP: TX power set successfully to {}dBm for role {}", tx_power, role);
    ResponseBuilder::build_ack()
}

pub async fn handle_start_rssi_reporting(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: START_RSSI_REPORTING - placeholder implementation");
    ResponseBuilder::build_ack()
}

pub async fn handle_stop_rssi_reporting(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: STOP_RSSI_REPORTING - placeholder implementation");
    ResponseBuilder::build_ack()
}
