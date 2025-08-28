//! GAP (Generic Access Profile) Commands Implementation
//! 
//! Handles GAP operations including advertising, connection management,
//! device configuration, and power management.

use defmt::debug;
use nrf_softdevice::{Softdevice, ble::{Address, AddressType}};

use crate::{
    buffer_pool::TxPacket,
    commands::{CommandError, ResponseBuilder},
    gap_state,
    protocol::serialization::{PayloadReader},
};

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
    
    response.build(crate::protocol::ResponseCode::Ack)
}

pub async fn handle_set_addr(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: SET_ADDR");
    
    if payload.len() < 7 { // addr_type (1) + addr (6)
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
    response.build(crate::protocol::ResponseCode::Ack)
}

pub async fn handle_adv_start(_payload: &[u8], _sd: &Softdevice) -> Result<TxPacket, CommandError> {
    debug!("GAP: ADV_START - placeholder implementation");
    ResponseBuilder::build_ack()
}

pub async fn handle_adv_stop(_payload: &[u8], _sd: &Softdevice) -> Result<TxPacket, CommandError> {
    debug!("GAP: ADV_STOP - placeholder implementation");
    ResponseBuilder::build_ack()
}

pub async fn handle_adv_configure(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: ADV_CONFIGURE - placeholder implementation");
    ResponseBuilder::build_ack()
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
    
    response.build(crate::protocol::ResponseCode::Ack)
}

pub async fn handle_set_name(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: SET_NAME");
    
    if payload.len() < 2 { // At least security mode (2 bytes)
        return ResponseBuilder::build_error(CommandError::InvalidPayload);
    }
    
    let mut reader = PayloadReader::new(payload);
    let sec_mode_sm = reader.read_u8()?; // Security mode sm field
    let sec_mode_lv = reader.read_u8()?; // Security mode lv field
    
    // Remaining payload is the device name
    let name_bytes = &payload[2..];
    
    // Set device name using SoftDevice API
    let sec_mode = nrf_softdevice::raw::ble_gap_conn_sec_mode_t {
        _bitfield_1: nrf_softdevice::raw::ble_gap_conn_sec_mode_t::new_bitfield_1(sec_mode_sm, sec_mode_lv)
    };
    
    let result = unsafe {
        nrf_softdevice::raw::sd_ble_gap_device_name_set(
            &sec_mode as *const _,
            name_bytes.as_ptr(),
            name_bytes.len() as u16
        )
    };
    
    // Update our internal state if successful
    if result == nrf_softdevice::raw::NRF_SUCCESS {
        let mut state = gap_state::gap_state().lock().await;
        state.set_device_name(name_bytes);
    }
    
    let mut response = ResponseBuilder::new();
    response.add_u32(result)?;
    response.build(crate::protocol::ResponseCode::Ack)
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
    
    let result = unsafe {
        nrf_softdevice::raw::sd_ble_gap_ppcp_get(&mut conn_params as *mut _)
    };
    
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
    
    response.build(crate::protocol::ResponseCode::Ack)
}

pub async fn handle_conn_params_set(payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: CONN_PARAMS_SET");
    
    if payload.len() < 8 { // 4 x u16 = 8 bytes
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
    
    let result = unsafe {
        nrf_softdevice::raw::sd_ble_gap_ppcp_set(&conn_params as *const _)
    };
    
    // Update our internal state if successful
    if result == nrf_softdevice::raw::NRF_SUCCESS {
        let mut state = gap_state::gap_state().lock().await;
        state.preferred_conn_params = crate::gap_state::ConnectionParams {
            min_conn_interval,
            max_conn_interval,
            slave_latency,
            conn_sup_timeout,
        };
    }
    
    let mut response = ResponseBuilder::new();
    response.add_u32(result)?;
    response.build(crate::protocol::ResponseCode::Ack)
}

pub async fn handle_conn_param_update(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: CONN_PARAM_UPDATE - placeholder implementation");
    ResponseBuilder::build_ack()
}

pub async fn handle_data_length_update(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: DATA_LENGTH_UPDATE - placeholder implementation");
    ResponseBuilder::build_ack()
}

pub async fn handle_phy_update(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: PHY_UPDATE - placeholder implementation");
    ResponseBuilder::build_ack()
}

pub async fn handle_disconnect(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: DISCONNECT - placeholder implementation");
    ResponseBuilder::build_ack()
}

pub async fn handle_set_tx_power(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    debug!("GAP: SET_TX_POWER - placeholder implementation");
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