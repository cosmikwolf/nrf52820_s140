//! GAP State Management
//!
//! Memory-optimized state management for GAP operations.
//! Uses packed structures and static allocation to minimize RAM usage.

use defmt::Format;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

/// Maximum device name length (GAP specification limit)
pub const MAX_DEVICE_NAME_LEN: usize = 32;

/// Maximum advertising data length (BLE specification)
pub const MAX_ADV_DATA_LEN: usize = 31;

/// Advertising state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
#[repr(u8)]
pub enum AdvState {
    Stopped = 0,
    Starting = 1,
    Active = 2,
    Stopping = 3,
}

/// Connection parameters structure (matches SoftDevice ble_gap_conn_params_t)
#[derive(Debug, Clone, Copy, Format)]
#[repr(C)]
pub struct ConnectionParams {
    pub min_conn_interval: u16, // Connection interval minimum (1.25ms units)
    pub max_conn_interval: u16, // Connection interval maximum (1.25ms units)
    pub slave_latency: u16,     // Slave latency
    pub conn_sup_timeout: u16,  // Connection supervisory timeout (10ms units)
}

impl Default for ConnectionParams {
    fn default() -> Self {
        Self {
            min_conn_interval: 24, // 30ms
            max_conn_interval: 40, // 50ms
            slave_latency: 0,
            conn_sup_timeout: 400, // 4s
        }
    }
}

/// Memory-optimized GAP state structure
/// Total size: ~152 bytes (well within 2KB budget)
#[repr(C)]
pub struct GapState {
    // Device Identity (39 bytes)
    pub device_name: [u8; MAX_DEVICE_NAME_LEN], // 32 bytes
    pub device_name_len: u8,                    // Actual name length
    pub device_addr: [u8; 6],                   // BLE address
    pub addr_type: u8,                          // 0=Public, 1=Random

    // Advertising Configuration (71 bytes)
    pub adv_data: [u8; MAX_ADV_DATA_LEN],      // 31 bytes
    pub scan_rsp_data: [u8; MAX_ADV_DATA_LEN], // 31 bytes
    pub adv_data_len: u8,                      // Actual advertising data length
    pub scan_rsp_len: u8,                      // Actual scan response length
    pub adv_handle: u8,                        // Advertising set handle
    pub adv_state: u8,                         // Current advertising state
    pub adv_interval_min: u16,                 // Advertising interval min (0.625ms units)
    pub adv_interval_max: u16,                 // Advertising interval max (0.625ms units)
    pub adv_timeout: u16,                      // Advertising timeout (10ms units, 0=no timeout)

    // Connection Parameters (8 bytes)
    pub preferred_conn_params: ConnectionParams,

    // Connection State (12 bytes)
    pub conn_handle: u16,   // Connection handle (0xFFFF if disconnected)
    pub peer_addr: [u8; 6], // Peer device address
    pub peer_addr_type: u8, // Peer address type
    pub current_mtu: u16,   // Current ATT MTU
    pub tx_power: i8,       // Current TX power (dBm)

    // Status Flags (1 byte)
    pub status_flags: u8, // Bit-packed status flags
}

// Status flag bit positions
pub const FLAG_CONNECTED: u8 = 0x01;
pub const FLAG_RSSI_REPORTING: u8 = 0x02;
pub const FLAG_BONDED: u8 = 0x04;
pub const FLAG_ENCRYPTED: u8 = 0x08;

impl Default for GapState {
    fn default() -> Self {
        Self {
            // Initialize device name to "BLE_Modem"
            device_name: {
                let mut name = [0u8; MAX_DEVICE_NAME_LEN];
                let default_name = b"BLE_Modem";
                name[..default_name.len()].copy_from_slice(default_name);
                name
            },
            device_name_len: 9, // Length of "BLE_Modem"
            device_addr: [0; 6],
            addr_type: 0, // Public address

            adv_data: [0; MAX_ADV_DATA_LEN],
            scan_rsp_data: [0; MAX_ADV_DATA_LEN],
            adv_data_len: 0,
            scan_rsp_len: 0,
            adv_handle: 0,
            adv_state: AdvState::Stopped as u8,
            adv_interval_min: 160, // 100ms
            adv_interval_max: 320, // 200ms
            adv_timeout: 0,        // No timeout

            preferred_conn_params: ConnectionParams::default(),

            conn_handle: 0xFFFF, // Invalid handle = disconnected
            peer_addr: [0; 6],
            peer_addr_type: 0,
            current_mtu: 23, // Default ATT MTU
            tx_power: 0,     // 0 dBm

            status_flags: 0,
        }
    }
}

impl GapState {
    /// Initialize a new GAP state with defaults
    pub const fn new() -> Self {
        Self {
            device_name: [0; MAX_DEVICE_NAME_LEN],
            device_name_len: 0,
            device_addr: [0; 6],
            addr_type: 0,

            adv_data: [0; MAX_ADV_DATA_LEN],
            scan_rsp_data: [0; MAX_ADV_DATA_LEN],
            adv_data_len: 0,
            scan_rsp_len: 0,
            adv_handle: 0,
            adv_state: AdvState::Stopped as u8,
            adv_interval_min: 160,
            adv_interval_max: 320,
            adv_timeout: 0,

            preferred_conn_params: ConnectionParams {
                min_conn_interval: 24,
                max_conn_interval: 40,
                slave_latency: 0,
                conn_sup_timeout: 400,
            },

            conn_handle: 0xFFFF,
            peer_addr: [0; 6],
            peer_addr_type: 0,
            current_mtu: 23,
            tx_power: 0,

            status_flags: 0,
        }
    }

    /// Get current advertising state
    pub fn adv_state(&self) -> AdvState {
        match self.adv_state {
            0 => AdvState::Stopped,
            1 => AdvState::Starting,
            2 => AdvState::Active,
            3 => AdvState::Stopping,
            _ => AdvState::Stopped, // Default to stopped for invalid values
        }
    }

    /// Set advertising state
    pub fn set_adv_state(&mut self, state: AdvState) {
        self.adv_state = state as u8;
    }

    /// Check if currently connected
    pub fn is_connected(&self) -> bool {
        (self.status_flags & FLAG_CONNECTED) != 0
    }

    /// Set connection status
    pub fn set_connected(&mut self, connected: bool) {
        if connected {
            self.status_flags |= FLAG_CONNECTED;
        } else {
            self.status_flags &= !FLAG_CONNECTED;
            self.conn_handle = 0xFFFF;
        }
    }

    /// Set device name (truncated to MAX_DEVICE_NAME_LEN if too long)
    pub fn set_device_name(&mut self, name: &[u8]) {
        let len = name.len().min(MAX_DEVICE_NAME_LEN);
        self.device_name[..len].copy_from_slice(&name[..len]);
        self.device_name_len = len as u8;

        // Clear remaining bytes
        if len < MAX_DEVICE_NAME_LEN {
            self.device_name[len..].fill(0);
        }
    }

    /// Get device name as slice
    pub fn device_name(&self) -> &[u8] {
        &self.device_name[..self.device_name_len as usize]
    }

    /// Set advertising data (truncated if too long)
    pub fn set_adv_data(&mut self, data: &[u8]) {
        let len = data.len().min(MAX_ADV_DATA_LEN);
        self.adv_data[..len].copy_from_slice(&data[..len]);
        self.adv_data_len = len as u8;

        // Clear remaining bytes
        if len < MAX_ADV_DATA_LEN {
            self.adv_data[len..].fill(0);
        }
    }

    /// Get advertising data as slice
    pub fn adv_data(&self) -> &[u8] {
        &self.adv_data[..self.adv_data_len as usize]
    }

    /// Set scan response data (truncated if too long)
    pub fn set_scan_response(&mut self, data: &[u8]) {
        let len = data.len().min(MAX_ADV_DATA_LEN);
        self.scan_rsp_data[..len].copy_from_slice(&data[..len]);
        self.scan_rsp_len = len as u8;

        // Clear remaining bytes
        if len < MAX_ADV_DATA_LEN {
            self.scan_rsp_data[len..].fill(0);
        }
    }

    /// Get scan response data as slice
    pub fn scan_response(&self) -> &[u8] {
        &self.scan_rsp_data[..self.scan_rsp_len as usize]
    }
}

/// Global GAP state - single static instance to minimize memory usage
static GAP_STATE: Mutex<CriticalSectionRawMutex, GapState> = Mutex::new(GapState::new());

/// Get reference to global GAP state
pub fn gap_state() -> &'static Mutex<CriticalSectionRawMutex, GapState> {
    &GAP_STATE
}

/// Initialize GAP state with default device name
pub async fn init() {
    let mut state = GAP_STATE.lock().await;
    state.set_device_name(b"BLE_Modem");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gap_state_size() {
        // Verify our memory budget
        assert!(
            core::mem::size_of::<GapState>() < 200,
            "GapState too large: {} bytes",
            core::mem::size_of::<GapState>()
        );
    }

    #[test]
    fn test_device_name() {
        let mut state = GapState::default();

        // Test normal name
        state.set_device_name(b"Test Device");
        assert_eq!(state.device_name(), b"Test Device");
        assert_eq!(state.device_name_len, 11);

        // Test truncation
        let long_name = b"This is a very long device name that should be truncated";
        state.set_device_name(long_name);
        assert_eq!(state.device_name_len, MAX_DEVICE_NAME_LEN as u8);
        assert_eq!(state.device_name(), &long_name[..MAX_DEVICE_NAME_LEN]);
    }

    #[test]
    fn test_advertising_state() {
        let mut state = GapState::default();

        assert_eq!(state.adv_state(), AdvState::Stopped);

        state.set_adv_state(AdvState::Active);
        assert_eq!(state.adv_state(), AdvState::Active);
    }

    #[test]
    fn test_connection_flags() {
        let mut state = GapState::default();

        assert!(!state.is_connected());

        state.set_connected(true);
        assert!(state.is_connected());
        assert_eq!(state.status_flags & FLAG_CONNECTED, FLAG_CONNECTED);

        state.set_connected(false);
        assert!(!state.is_connected());
        assert_eq!(state.conn_handle, 0xFFFF);
    }
}
