//! BLE Modem State Management
//! 
//! This module manages the global state of the BLE modem including:
//! - UUID base registrations
//! - Connection state
//! - Advertising state
//! - Dynamic GATT services and characteristics

use defmt::Format;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use heapless::{IndexMap, Vec};
use core::str::FromStr;
use nrf_softdevice::{
    ble::{Connection, Uuid},
    Softdevice,
};

/// Maximum number of registered UUID bases (matches original C implementation)
pub const MAX_UUID_BASES: usize = 4;

/// Maximum number of services
pub const MAX_SERVICES: usize = 16;

/// Maximum number of characteristics per service
pub const MAX_CHARACTERISTICS: usize = 64;

/// UUID base entry
#[derive(Debug, Clone, Copy, Format)]
pub struct UuidBase {
    pub base: [u8; 16],
    pub handle: u8,
}

/// Service information
#[derive(Debug, Clone, Copy, Format)]
pub struct ServiceInfo {
    pub handle: u16,
    pub uuid: Uuid,
    pub service_type: ServiceType,
}

/// Characteristic information
#[derive(Debug, Clone, Copy, Format)]
pub struct CharacteristicInfo {
    pub service_handle: u16,
    pub value_handle: u16,
    pub cccd_handle: u16,
    pub sccd_handle: u16,
    pub uuid: Uuid,
    pub properties: u8,
}

/// Service type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum ServiceType {
    Primary = 1,
    Secondary = 2,
}

/// Connection parameters
#[derive(Debug, Clone, Copy, Format)]
pub struct ConnectionParams {
    pub min_conn_interval: u16,
    pub max_conn_interval: u16,
    pub slave_latency: u16,
    pub conn_sup_timeout: u16,
}

/// Advertising state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum AdvertisingState {
    Stopped,
    Starting,
    Active,
    Stopping,
}

/// Connection state
#[derive(Debug, Clone, Copy, Format)]
pub struct ConnectionState {
    pub connected: bool,
    pub conn_handle: u16,
    pub peer_addr: [u8; 6],
    pub peer_addr_type: u8,
    pub mtu: u16,
    pub rssi_reporting: bool,
}

/// Device configuration
#[derive(Debug, Clone, Format)]
pub struct DeviceConfig {
    pub device_name: heapless::String<32>,
    pub device_addr: [u8; 6],
    pub addr_type: u8,
    pub tx_power: i8,
    pub preferred_conn_params: ConnectionParams,
}

/// Main modem state structure
#[derive(Format)]
pub struct ModemState {
    /// Registered UUID bases
    pub uuid_bases: Vec<UuidBase, MAX_UUID_BASES>,
    
    /// Dynamic services
    pub services: Vec<ServiceInfo, MAX_SERVICES>,
    
    /// Dynamic characteristics
    pub characteristics: Vec<CharacteristicInfo, MAX_CHARACTERISTICS>,
    
    /// Current connection state
    pub connection: Option<ConnectionState>,
    
    /// Current advertising state
    pub advertising_state: AdvertisingState,
    
    /// Device configuration
    pub device_config: DeviceConfig,
    
    /// Characteristic handle to service handle mapping
    pub char_to_service_map: IndexMap<u16, u16, MAX_CHARACTERISTICS>,
}

impl Default for ConnectionParams {
    fn default() -> Self {
        Self {
            min_conn_interval: 24,     // 30ms (24 * 1.25ms)
            max_conn_interval: 40,     // 50ms (40 * 1.25ms)
            slave_latency: 0,
            conn_sup_timeout: 400,     // 4s (400 * 10ms)
        }
    }
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            device_name: heapless::String::try_from("BLE-Modem").unwrap(),
            device_addr: [0; 6],
            addr_type: 0,
            tx_power: 0,
            preferred_conn_params: ConnectionParams::default(),
        }
    }
}

impl ModemState {
    /// Create a new modem state
    pub fn new() -> Self {
        Self {
            uuid_bases: Vec::new(),
            services: Vec::new(),
            characteristics: Vec::new(),
            connection: None,
            advertising_state: AdvertisingState::Stopped,
            device_config: DeviceConfig::default(),
            char_to_service_map: IndexMap::new(),
        }
    }

    /// Register a new UUID base
    pub fn register_uuid_base(&mut self, base: [u8; 16]) -> Result<u8, StateError> {
        if self.uuid_bases.is_full() {
            return Err(StateError::UuidBasesExhausted);
        }

        let handle = self.uuid_bases.len() as u8;
        let uuid_base = UuidBase { base, handle };
        
        self.uuid_bases.push(uuid_base)
            .map_err(|_| StateError::UuidBasesExhausted)?;

        Ok(handle)
    }

    /// Get UUID base by handle
    pub fn get_uuid_base(&self, handle: u8) -> Option<&UuidBase> {
        self.uuid_bases.get(handle as usize)
    }

    /// Add a new service
    pub fn add_service(&mut self, handle: u16, uuid: Uuid, service_type: ServiceType) -> Result<(), StateError> {
        if self.services.is_full() {
            return Err(StateError::ServicesExhausted);
        }

        let service_info = ServiceInfo {
            handle,
            uuid,
            service_type,
        };

        self.services.push(service_info)
            .map_err(|_| StateError::ServicesExhausted)
    }

    /// Get service by handle
    pub fn get_service(&self, handle: u16) -> Option<&ServiceInfo> {
        self.services.iter().find(|s| s.handle == handle)
    }

    /// Add a new characteristic
    pub fn add_characteristic(&mut self, char_info: CharacteristicInfo) -> Result<(), StateError> {
        if self.characteristics.is_full() {
            return Err(StateError::CharacteristicsExhausted);
        }

        // Add to handle mapping
        self.char_to_service_map.insert(char_info.value_handle, char_info.service_handle)
            .map_err(|_| StateError::CharacteristicsExhausted)?;

        self.characteristics.push(char_info)
            .map_err(|_| StateError::CharacteristicsExhausted)
    }

    /// Get characteristic by value handle
    pub fn get_characteristic_by_handle(&self, handle: u16) -> Option<&CharacteristicInfo> {
        self.characteristics.iter().find(|c| c.value_handle == handle)
    }

    /// Get service handle for characteristic handle
    pub fn get_service_handle_for_char(&self, char_handle: u16) -> Option<u16> {
        self.char_to_service_map.get(&char_handle).copied()
    }

    /// Update connection state
    pub fn set_connection(&mut self, conn_state: Option<ConnectionState>) {
        self.connection = conn_state;
    }

    /// Get connection state
    pub fn get_connection(&self) -> Option<&ConnectionState> {
        self.connection.as_ref()
    }

    /// Set advertising state
    pub fn set_advertising_state(&mut self, state: AdvertisingState) {
        self.advertising_state = state;
    }

    /// Get advertising state
    pub fn get_advertising_state(&self) -> AdvertisingState {
        self.advertising_state
    }

    /// Update device name
    pub fn set_device_name(&mut self, name: &str) -> Result<(), StateError> {
        self.device_config.device_name = heapless::String::try_from(name)
            .map_err(|_| StateError::NameTooLong)?;
        Ok(())
    }

    /// Get device name
    pub fn get_device_name(&self) -> &str {
        &self.device_config.device_name
    }

    /// Set device address
    pub fn set_device_address(&mut self, addr: [u8; 6], addr_type: u8) {
        self.device_config.device_addr = addr;
        self.device_config.addr_type = addr_type;
    }

    /// Get device address
    pub fn get_device_address(&self) -> ([u8; 6], u8) {
        (self.device_config.device_addr, self.device_config.addr_type)
    }

    /// Set TX power
    pub fn set_tx_power(&mut self, power: i8) {
        self.device_config.tx_power = power;
    }

    /// Get TX power
    pub fn get_tx_power(&self) -> i8 {
        self.device_config.tx_power
    }

    /// Set preferred connection parameters
    pub fn set_preferred_conn_params(&mut self, params: ConnectionParams) {
        self.device_config.preferred_conn_params = params;
    }

    /// Get preferred connection parameters
    pub fn get_preferred_conn_params(&self) -> &ConnectionParams {
        &self.device_config.preferred_conn_params
    }

    /// Clear all dynamic GATT data (for reset)
    pub fn clear_gatt_data(&mut self) {
        self.services.clear();
        self.characteristics.clear();
        self.char_to_service_map.clear();
    }
}

impl Default for ModemState {
    fn default() -> Self {
        Self::new()
    }
}

/// State management errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum StateError {
    UuidBasesExhausted,
    ServicesExhausted,
    CharacteristicsExhausted,
    NameTooLong,
    InvalidHandle,
}

/// Global modem state - protected by mutex for thread safety
pub static MODEM_STATE: Mutex<NoopRawMutex, ModemState> = Mutex::new(ModemState::new());

/// Helper functions for state access
pub async fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut ModemState) -> R,
{
    let mut state = MODEM_STATE.lock().await;
    f(&mut state)
}

/// Initialize the modem state
pub fn init() {
    defmt::info!("Modem state initialized");
}