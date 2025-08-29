//! BLE Advertising Controller
//! 
//! Bridges between protocol GAP commands and nrf-softdevice high-level APIs.
//! Provides coordinated advertising management that can be controlled via
//! individual commands while leveraging the robust high-level abstractions.

use defmt::debug;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, channel::Channel};
use embassy_time::{Duration, Timer};
use heapless::Vec;
use nrf_softdevice::{
    ble::{
        peripheral::{self, ConnectableAdvertisement, Config as PeripheralConfig, FilterPolicy},
        Phy, TxPower,
    },
    Softdevice,
};

use crate::{
    connection_manager,
    gap_state::{self, AdvState, MAX_ADV_DATA_LEN},
    services::Server,
};

/// Maximum advertising data length for static buffers
const MAX_COMBINED_ADV_DATA: usize = MAX_ADV_DATA_LEN * 2; // adv + scan response

/// Advertising command types
#[derive(Debug, Clone, Copy)]
pub enum AdvCommand {
    Start { handle: u8, conn_cfg_tag: u8 },
    Stop { handle: u8 },
    Configure { handle: u8, data_present: bool },
}

/// Advertising controller state
pub struct AdvController {
    /// Current advertising configuration
    config: PeripheralConfig,
    /// Combined advertising + scan response data buffer
    combined_data: Vec<u8, MAX_COMBINED_ADV_DATA>,
    /// Split point between adv data and scan response data  
    adv_data_len: usize,
    /// Whether advertising is currently requested
    advertising_requested: bool,
    /// Current advertising handle
    handle: u8,
}

impl AdvController {
    const fn new() -> Self {
        // Const-compatible config initialization
        let config = PeripheralConfig {
            primary_phy: Phy::M1,
            secondary_phy: Phy::M1,
            tx_power: TxPower::ZerodBm,
            timeout: None,
            max_events: None,
            interval: 400, // 250ms (same as default)
            filter_policy: FilterPolicy::Any,
        };
        
        Self {
            config,
            combined_data: Vec::new(),
            adv_data_len: 0,
            advertising_requested: false,
            handle: 0,
        }
    }
}

impl AdvController {
    /// Update advertising data configuration
    pub fn configure_data(&mut self, adv_data: &[u8], scan_data: &[u8]) -> Result<(), ()> {
        self.combined_data.clear();
        
        // Store advertising data first
        if self.combined_data.extend_from_slice(adv_data).is_err() {
            return Err(());
        }
        self.adv_data_len = adv_data.len();
        
        // Store scan response data
        if self.combined_data.extend_from_slice(scan_data).is_err() {
            return Err(());
        }
        
        Ok(())
    }
    
    /// Get current advertising data slice
    pub fn adv_data(&self) -> &[u8] {
        &self.combined_data[..self.adv_data_len]
    }
    
    /// Get current scan response data slice
    pub fn scan_data(&self) -> &[u8] {
        &self.combined_data[self.adv_data_len..]
    }
    
    /// Request advertising start
    pub fn start_advertising(&mut self, handle: u8, _conn_cfg_tag: u8) {
        self.advertising_requested = true;
        self.handle = handle;
        debug!("Advertising start requested for handle {}", handle);
    }
    
    /// Request advertising stop
    pub fn stop_advertising(&mut self, handle: u8) {
        if self.handle == handle {
            self.advertising_requested = false;
            debug!("Advertising stop requested for handle {}", handle);
        }
    }
    
    /// Check if advertising is currently requested
    pub fn is_advertising_requested(&self) -> bool {
        self.advertising_requested
    }
    
    /// Get current configuration
    pub fn config(&self) -> &PeripheralConfig {
        &self.config
    }
    
    /// Update advertising configuration
    pub fn update_config(&mut self, config: PeripheralConfig) {
        self.config = config;
    }
}

/// Global advertising controller instance
static ADV_CONTROLLER: Mutex<CriticalSectionRawMutex, AdvController> = 
    Mutex::new(AdvController::new());

/// Command channel for advertising control
static ADV_COMMAND_CHANNEL: Channel<CriticalSectionRawMutex, AdvCommand, 4> = Channel::new();

/// Get reference to global advertising controller
pub async fn controller() -> embassy_sync::mutex::MutexGuard<'static, CriticalSectionRawMutex, AdvController> {
    ADV_CONTROLLER.lock().await
}

/// Send advertising command (non-blocking)
pub fn send_command(cmd: AdvCommand) -> Result<(), AdvCommand> {
    ADV_COMMAND_CHANNEL.try_send(cmd).map_err(|e| match e {
        embassy_sync::channel::TrySendError::Full(cmd) => cmd,
    })
}

/// Enhanced BLE advertising task that coordinates with protocol commands
#[embassy_executor::task]
pub async fn advertising_task(sd: &'static Softdevice, bt_server: Server) {
    debug!("Starting coordinated advertising task...");
    
    // Default advertising data - can be overridden by ADV_CONFIGURE commands
    let mut default_adv_data = Vec::<u8, MAX_ADV_DATA_LEN>::new();
    let default_scan_data = Vec::<u8, MAX_ADV_DATA_LEN>::new();
    
    // Simple default advertisement
    let _ = default_adv_data.extend_from_slice(&[
        0x02, 0x01, 0x06,  // Flags: General Discoverable + LE Only
        0x0A, 0x09, b'B', b'L', b'E', b'_', b'M', b'o', b'd', b'e', b'm',  // Complete local name
    ]);
    
    loop {
        // Check for advertising commands
        if let Ok(cmd) = ADV_COMMAND_CHANNEL.try_receive() {
            let mut controller = ADV_CONTROLLER.lock().await;
            
            match cmd {
                AdvCommand::Start { handle, conn_cfg_tag } => {
                    controller.start_advertising(handle, conn_cfg_tag);
                    
                    // Update gap state
                    let mut gap_state = gap_state::gap_state().lock().await;
                    gap_state.set_adv_state(AdvState::Starting);
                    gap_state.adv_handle = handle;
                }
                AdvCommand::Stop { handle } => {
                    controller.stop_advertising(handle);
                    
                    // Update gap state  
                    let mut gap_state = gap_state::gap_state().lock().await;
                    gap_state.set_adv_state(AdvState::Stopping);
                }
                AdvCommand::Configure { handle, data_present } => {
                    if data_present {
                        // Get advertising data from gap state
                        let gap_state = gap_state::gap_state().lock().await;
                        let adv_data = gap_state.adv_data();
                        let scan_data = gap_state.scan_response();
                        
                        if controller.configure_data(adv_data, scan_data).is_ok() {
                            debug!("Advertising data configured for handle {}", handle);
                        }
                    }
                    controller.handle = handle;
                }
            }
        }
        
        // Check if advertising is requested
        let should_advertise = {
            let controller = ADV_CONTROLLER.lock().await;
            controller.is_advertising_requested()
        };
        
        if should_advertise {
            let (adv_data, scan_data, config) = {
                let controller = ADV_CONTROLLER.lock().await;
                let adv = controller.adv_data();
                let scan = controller.scan_data();
                
                // Use configured data if available, otherwise use defaults
                let adv_data = if !adv.is_empty() { 
                    Vec::from_slice(adv).unwrap_or_default()
                } else {
                    default_adv_data.clone()
                };
                
                let scan_data = if !scan.is_empty() {
                    Vec::from_slice(scan).unwrap_or_default()
                } else {
                    default_scan_data.clone()
                };
                
                (adv_data, scan_data, *controller.config())
            };
            
            // Update gap state to active
            {
                let mut gap_state = gap_state::gap_state().lock().await;
                gap_state.set_adv_state(AdvState::Active);
            }
            
            // Create advertising configuration
            let advertisement = ConnectableAdvertisement::ScannableUndirected {
                adv_data: &adv_data,
                scan_data: &scan_data,
            };
            
            // Start advertising and wait for connection
            match peripheral::advertise_connectable(sd, advertisement, &config).await {
                Ok(conn) => {
                    debug!("BLE connection established!");
                    
                    // Get connection handle and MTU
                    let conn_handle = conn.handle().unwrap_or(0);
                    let mtu = 23; // Default ATT MTU
                    
                    // Register connection with connection manager
                    if let Err(e) = connection_manager::with_connection_manager(|mgr| {
                        mgr.add_connection(conn_handle, mtu)
                    }) {
                        debug!("Failed to register connection: {:?}", e);
                    }
                    
                    // Update states
                    {
                        let mut gap_state = gap_state::gap_state().lock().await;
                        gap_state.set_connected(true);
                        gap_state.conn_handle = conn_handle;
                    }
                    {
                        let mut controller = ADV_CONTROLLER.lock().await;
                        controller.advertising_requested = false; // Stop advertising when connected
                    }
                    
                    // Run GATT server on the connection with event forwarding
                    use nrf_softdevice::ble::gatt_server;
                    
                    // Forward connection event to host
                    let connected_event = crate::events::create_connected_event(&conn);
                    if let Err(_) = crate::events::forward_event_to_host(connected_event).await {
                        debug!("Failed to forward connection event to host");
                    }
                    
                    let result = gatt_server::run(&conn, &bt_server, |event| {
                        // Forward GATT server events to host
                        debug!("GATT server event received: {:?}", defmt::Debug2Format(&event));
                        
                        // Note: We can't await in this closure, so event forwarding
                        // is handled in the Server::on_write implementation
                    }).await;
                    debug!("GATT server connection ended: {:?}", defmt::Debug2Format(&result));
                    
                    // Unregister connection from connection manager
                    let disconnection_reason = 0x13; // BLE_HCI_REMOTE_USER_TERMINATED_CONNECTION
                    if let Err(e) = connection_manager::with_connection_manager(|mgr| {
                        mgr.remove_connection(conn_handle, disconnection_reason)
                    }) {
                        debug!("Failed to unregister connection: {:?}", e);
                    }
                    
                    // Forward disconnection event to host
                    let disconnected_event = crate::events::create_disconnected_event(
                        conn_handle,
                        disconnection_reason
                    );
                    if let Err(_) = crate::events::forward_event_to_host(disconnected_event).await {
                        debug!("Failed to forward disconnection event to host");
                    }
                    
                    // Update connection state
                    {
                        let mut gap_state = gap_state::gap_state().lock().await;
                        gap_state.set_connected(false);
                    }
                }
                Err(e) => {
                    debug!("Advertising failed: {:?}", defmt::Debug2Format(&e));
                    
                    // Update gap state to stopped on error
                    {
                        let mut gap_state = gap_state::gap_state().lock().await;
                        gap_state.set_adv_state(AdvState::Stopped);
                    }
                    {
                        let mut controller = ADV_CONTROLLER.lock().await;
                        controller.advertising_requested = false;
                    }
                    
                    Timer::after(Duration::from_secs(1)).await;
                }
            }
        } else {
            // Not advertising - update gap state if needed
            {
                let gap_state = gap_state::gap_state().lock().await;
                if gap_state.adv_state() != AdvState::Stopped {
                    drop(gap_state);
                    let mut gap_state = gap_state::gap_state().lock().await;
                    gap_state.set_adv_state(AdvState::Stopped);
                }
            }
            
            // Brief delay when not advertising
            Timer::after(Duration::from_millis(100)).await;
        }
    }
}