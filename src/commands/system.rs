//! System Commands Implementation
//! 
//! Handles system-level commands:
//! - REQ_GET_INFO: Get firmware version
//! - REQ_SHUTDOWN: Power down system  
//! - REQ_REBOOT: System reset

use defmt::{info, warn};

use crate::{
    buffer_pool::TxPacket,
    commands::{CommandError, ResponseBuilder},
};

/// Firmware version in BCD format (matches original C implementation)
const FIRMWARE_VERSION_BCD: u32 = 0x00010000; // Version 1.0.0.0

/// Handle GET_INFO command (0x0001)
/// Returns firmware version in BCD format
pub async fn handle_get_info(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    info!("System: GET_INFO requested");
    
    let mut response = ResponseBuilder::new();
    response.add_u32(FIRMWARE_VERSION_BCD)?;
    
    info!("System: Returning firmware version 0x{:08X}", FIRMWARE_VERSION_BCD);
    response.build(crate::protocol::ResponseCode::Ack)
}

/// Handle SHUTDOWN command (0x0002)
/// Powers down the system
pub async fn handle_shutdown(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    warn!("System: SHUTDOWN requested");
    
    // Send ACK first
    let response = ResponseBuilder::build_ack()?;
    
    // Note: In a real implementation, we would initiate system shutdown here
    // This might involve:
    // - Gracefully closing BLE connections
    // - Stopping advertising
    // - Entering deep sleep or system off mode
    // - Potentially using nrf_softdevice::raw::sd_power_system_off()
    
    warn!("System: Shutdown ACK sent - system should power down");
    
    // For now, we just acknowledge the command
    // In a production system, this would trigger actual shutdown
    
    Ok(response)
}

/// Handle REBOOT command (0x00F0)
/// Performs system reset
pub async fn handle_reboot(_payload: &[u8]) -> Result<TxPacket, CommandError> {
    warn!("System: REBOOT requested");
    
    // Send ACK first
    let response = ResponseBuilder::build_ack()?;
    
    // Note: In a real implementation, we would initiate system reset here
    // This could be done using:
    // - cortex_m::peripheral::SCB::sys_reset()
    // - nrf_softdevice::raw::sd_nvic_SystemReset()
    // - embassy_nrf's reset functionality
    
    warn!("System: Reboot ACK sent - system should reset");
    
    // For safety during development, we don't actually reset
    // In production, uncomment the following line:
    // cortex_m::peripheral::SCB::sys_reset();
    
    Ok(response)
}