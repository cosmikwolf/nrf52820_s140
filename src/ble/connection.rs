//! Connection Management
//!
//! Manages BLE connections for the dynamic GATT system.
//! Provides connection storage, handle mapping, and event forwarding.

use defmt::{debug, error, Format};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use heapless::index_map::FnvIndexMap;

/// Maximum number of simultaneous connections
pub const MAX_CONNECTIONS: usize = 2;

/// Connection information
#[derive(Format, Clone)]
pub struct ConnectionInfo {
    /// Connection handle
    pub handle: u16,
    /// Current MTU size
    pub mtu: u16,
    /// Connection parameters
    pub conn_params: ConnectionParams,
}

/// Connection parameters
#[derive(Format, Clone, Copy)]
pub struct ConnectionParams {
    /// Minimum connection interval (units of 1.25ms)
    pub min_conn_interval: u16,
    /// Maximum connection interval (units of 1.25ms)
    pub max_conn_interval: u16,
    /// Slave latency
    pub slave_latency: u16,
    /// Supervision timeout (units of 10ms)
    pub supervision_timeout: u16,
}

impl Default for ConnectionParams {
    fn default() -> Self {
        Self {
            min_conn_interval: 24, // 30ms
            max_conn_interval: 40, // 50ms
            slave_latency: 0,
            supervision_timeout: 400, // 4s
        }
    }
}

/// Connection events that need to be forwarded to host
#[derive(Format, Clone)]
pub enum ConnectionEvent {
    Connected { handle: u16, params: ConnectionParams },
    Disconnected { handle: u16, reason: u8 },
    ParamsUpdated { handle: u16, params: ConnectionParams },
    MtuChanged { handle: u16, mtu: u16 },
}

/// Connection manager state
pub struct ConnectionManager {
    /// Active connections indexed by handle
    connections: FnvIndexMap<u16, ConnectionInfo, MAX_CONNECTIONS>,
    /// Event sender for forwarding to host
    event_sender: Option<Sender<'static, CriticalSectionRawMutex, ConnectionEvent, 8>>,
}

impl ConnectionManager {
    pub const fn new() -> Self {
        Self {
            connections: FnvIndexMap::new(),
            event_sender: None,
        }
    }

    /// Set the event sender for forwarding events to host
    pub fn set_event_sender(&mut self, sender: Sender<'static, CriticalSectionRawMutex, ConnectionEvent, 8>) {
        self.event_sender = Some(sender);
    }

    /// Add a new connection
    pub fn add_connection(&mut self, handle: u16, mtu: u16) -> Result<(), ConnectionError> {
        // BLE connection handle 0 is reserved and invalid
        if handle == 0 {
            error!("CONNECTION: Invalid connection handle 0");
            return Err(ConnectionError::InvalidHandle);
        }

        let conn_info = ConnectionInfo {
            handle,
            mtu,
            conn_params: ConnectionParams::default(),
        };

        if self.connections.insert(handle, conn_info.clone()).is_err() {
            error!("CONNECTION: Failed to add connection {} - map full", handle);
            return Err(ConnectionError::ConnectionMapFull);
        }

        debug!("CONNECTION: Added connection {} with MTU {}", handle, mtu);

        // Forward connection event to host
        if let Some(sender) = &self.event_sender {
            let event = ConnectionEvent::Connected {
                handle,
                params: conn_info.conn_params,
            };
            if sender.try_send(event).is_err() {
                error!("CONNECTION: Failed to forward connection event - queue full");
            }
        }

        Ok(())
    }

    /// Remove a connection
    pub fn remove_connection(&mut self, handle: u16, reason: u8) -> Result<(), ConnectionError> {
        if self.connections.remove(&handle).is_none() {
            error!("CONNECTION: Attempted to remove unknown connection {}", handle);
            return Err(ConnectionError::ConnectionNotFound);
        }

        debug!("CONNECTION: Removed connection {} (reason: {})", handle, reason);

        // Forward disconnection event to host
        if let Some(sender) = &self.event_sender {
            let event = ConnectionEvent::Disconnected { handle, reason };
            if sender.try_send(event).is_err() {
                error!("CONNECTION: Failed to forward disconnection event - queue full");
            }
        }

        Ok(())
    }

    /// Get connection info by handle
    pub fn get_connection(&self, handle: u16) -> Option<&ConnectionInfo> {
        self.connections.get(&handle)
    }

    /// Check if a connection exists
    pub fn is_connected(&self, handle: u16) -> bool {
        self.connections.contains_key(&handle)
    }

    /// Get the number of active connections
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Update connection MTU
    pub fn update_mtu(&mut self, handle: u16, mtu: u16) -> Result<(), ConnectionError> {
        match self.connections.get_mut(&handle) {
            Some(conn) => {
                conn.mtu = mtu;
                debug!("CONNECTION: Updated MTU for connection {} to {}", handle, mtu);

                // Forward MTU change event to host
                if let Some(sender) = &self.event_sender {
                    let event = ConnectionEvent::MtuChanged { handle, mtu };
                    if sender.try_send(event).is_err() {
                        error!("CONNECTION: Failed to forward MTU change event - queue full");
                    }
                }

                Ok(())
            }
            None => {
                error!("CONNECTION: Attempted to update MTU for unknown connection {}", handle);
                Err(ConnectionError::ConnectionNotFound)
            }
        }
    }

    /// Update connection parameters
    pub fn update_params(&mut self, handle: u16, params: ConnectionParams) -> Result<(), ConnectionError> {
        match self.connections.get_mut(&handle) {
            Some(conn) => {
                conn.conn_params = params;
                debug!("CONNECTION: Updated parameters for connection {}", handle);

                // Forward parameter change event to host
                if let Some(sender) = &self.event_sender {
                    let event = ConnectionEvent::ParamsUpdated { handle, params };
                    if sender.try_send(event).is_err() {
                        error!("CONNECTION: Failed to forward params change event - queue full");
                    }
                }

                Ok(())
            }
            None => {
                error!(
                    "CONNECTION: Attempted to update params for unknown connection {}",
                    handle
                );
                Err(ConnectionError::ConnectionNotFound)
            }
        }
    }

    /// Get all active connection handles
    pub fn active_handles(&self) -> impl Iterator<Item = u16> + '_ {
        self.connections.keys().copied()
    }

}

/// Connection management errors
#[derive(Format, Debug, Clone, Copy)]
pub enum ConnectionError {
    ConnectionNotFound,
    ConnectionMapFull,
    InvalidHandle,
}

/// Global connection manager instance  
static mut CONNECTION_MANAGER: Option<ConnectionManager> = None;

/// Initialize the connection manager
pub fn init() {
    unsafe {
        CONNECTION_MANAGER = Some(ConnectionManager::new());
    }
}

/// Access the global connection manager
pub fn with_connection_manager<F, R>(f: F) -> R
where
    F: FnOnce(&mut ConnectionManager) -> R,
{
    cortex_m::interrupt::free(|_cs| unsafe { CONNECTION_MANAGER.as_mut().map(f).unwrap() })
}

/// Event channel for connection events
pub static CONNECTION_EVENT_CHANNEL: Channel<CriticalSectionRawMutex, ConnectionEvent, 8> = Channel::new();

/// Get the connection event receiver
pub fn connection_event_receiver() -> Receiver<'static, CriticalSectionRawMutex, ConnectionEvent, 8> {
    CONNECTION_EVENT_CHANNEL.receiver()
}

/// Get the connection event sender
pub fn connection_event_sender() -> Sender<'static, CriticalSectionRawMutex, ConnectionEvent, 8> {
    CONNECTION_EVENT_CHANNEL.sender()
}
