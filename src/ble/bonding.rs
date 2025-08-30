//! Bonding Service
//!
//! Manages BLE bonding and system attributes for persistent connections.
//! Handles CCCD states and other client-specific data.

use defmt::{debug, info, warn, Format};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::once_lock::OnceLock;
use heapless::index_map::FnvIndexMap;

/// Maximum number of bonded devices
pub const MAX_BONDED_DEVICES: usize = 2;

/// Maximum system attributes data size
pub const MAX_SYS_ATTR_SIZE: usize = 64;

/// Bonded device information
#[derive(Debug, Clone)]
pub struct BondedDevice {
    /// Connection handle (may change between connections)
    pub conn_handle: u16,
    /// Device address
    pub peer_addr: [u8; 6],
    /// Address type (public/random)
    pub addr_type: u8,
    /// System attributes data (CCCD states, etc.)
    pub sys_attr_data: heapless::Vec<u8, MAX_SYS_ATTR_SIZE>,
}

/// Bonding service errors
#[derive(Debug, Clone, Copy, Format)]
pub enum BondingError {
    BondingTableFull,
    DeviceNotFound,
    InvalidData,
}

/// Bonding storage
struct BondingStorage {
    /// Bonded devices indexed by connection handle
    bonded_devices: FnvIndexMap<u16, BondedDevice, MAX_BONDED_DEVICES>,
    /// Next available bond ID
    next_bond_id: u16,
}

impl BondingStorage {
    fn new() -> Self {
        let storage = Self {
            bonded_devices: FnvIndexMap::new(),
            next_bond_id: 1,
        };
        debug!(
            "BONDING: Created new BondingStorage with {} devices",
            storage.bonded_devices.len()
        );
        storage
    }

    fn add_bonded_device(&mut self, conn_handle: u16, peer_addr: [u8; 6], addr_type: u8) -> Result<(), BondingError> {
        debug!(
            "BONDING: Attempting to add device {} (current count: {}/{})",
            conn_handle,
            self.bonded_devices.len(),
            MAX_BONDED_DEVICES
        );

        let device = BondedDevice {
            conn_handle,
            peer_addr,
            addr_type,
            sys_attr_data: heapless::Vec::new(),
        };

        if self.bonded_devices.insert(conn_handle, device).is_err() {
            debug!(
                "BONDING: Cannot add device {} - table full ({}/{})",
                conn_handle,
                self.bonded_devices.len(),
                MAX_BONDED_DEVICES
            );
            return Err(BondingError::BondingTableFull);
        }

        debug!(
            "BONDING: Added bonded device for connection {} (new count: {})",
            conn_handle,
            self.bonded_devices.len()
        );
        Ok(())
    }

    fn set_system_attributes(&mut self, conn_handle: u16, sys_attr_data: &[u8]) -> Result<(), BondingError> {
        match self.bonded_devices.get_mut(&conn_handle) {
            Some(device) => {
                // Check size first before clearing existing data
                if sys_attr_data.len() > MAX_SYS_ATTR_SIZE {
                    debug!(
                        "BONDING: System attributes rejected for connection {} ({} > {} bytes)",
                        conn_handle,
                        sys_attr_data.len(),
                        MAX_SYS_ATTR_SIZE
                    );
                    return Err(BondingError::InvalidData);
                }

                // Size is valid, now update the data
                device.sys_attr_data.clear();
                let _ = device.sys_attr_data.extend_from_slice(sys_attr_data); // This should never fail now
                debug!(
                    "BONDING: Updated system attributes for connection {} ({} bytes)",
                    conn_handle,
                    sys_attr_data.len()
                );
                Ok(())
            }
            None => {
                warn!(
                    "BONDING: Attempted to set system attributes for unknown device {}",
                    conn_handle
                );
                Err(BondingError::DeviceNotFound)
            }
        }
    }

    fn get_system_attributes(&self, conn_handle: u16) -> Option<&[u8]> {
        self.bonded_devices
            .get(&conn_handle)
            .map(|device| device.sys_attr_data.as_slice())
    }

    fn remove_bonded_device(&mut self, conn_handle: u16) -> Result<(), BondingError> {
        if self.bonded_devices.remove(&conn_handle).is_none() {
            warn!("BONDING: Attempted to remove unknown bonded device {}", conn_handle);
            return Err(BondingError::DeviceNotFound);
        }

        debug!("BONDING: Removed bonded device for connection {}", conn_handle);
        Ok(())
    }

    fn device_count(&self) -> usize {
        let count = self.bonded_devices.len();
        debug!(
            "BONDING: device_count() = {} (map capacity: {})",
            count,
            self.bonded_devices.capacity()
        );
        if count > 0 {
            debug!("BONDING: device count > 0, checking individual handles...");
            for handle in 1..200u16 {
                if self.bonded_devices.contains_key(&handle) {
                    debug!("BONDING: Found device with handle {}", handle);
                }
            }
        }
        count
    }
}

/// Global bonding storage - protected by mutex for thread safety
static BONDING_STORAGE: OnceLock<Mutex<CriticalSectionRawMutex, BondingStorage>> = OnceLock::new();

/// Get or initialize the global bonding storage
fn get_bonding_storage() -> &'static Mutex<CriticalSectionRawMutex, BondingStorage> {
    BONDING_STORAGE.get_or_init(|| {
        debug!("BONDING: Initializing bonding storage for the first time");
        Mutex::new(BondingStorage::new())
    })
}

/// Initialize the bonding service
pub fn init() {
    let storage = get_bonding_storage();
    debug!("BONDING: Bonding service initialized");
}

/// Add a bonded device
pub async fn add_bonded_device(conn_handle: u16, peer_addr: [u8; 6], addr_type: u8) -> Result<(), BondingError> {
    let mut storage = get_bonding_storage().lock().await;
    storage.add_bonded_device(conn_handle, peer_addr, addr_type)
}

/// Set system attributes for a bonded device
pub async fn set_system_attributes(conn_handle: u16, sys_attr_data: &[u8]) -> Result<(), BondingError> {
    let mut storage = get_bonding_storage().lock().await;
    storage.set_system_attributes(conn_handle, sys_attr_data)
}

/// Get system attributes for a bonded device
pub async fn get_system_attributes(conn_handle: u16) -> Option<heapless::Vec<u8, MAX_SYS_ATTR_SIZE>> {
    let storage = get_bonding_storage().lock().await;
    storage.get_system_attributes(conn_handle)
        .map(|data| {
            let mut vec = heapless::Vec::new();
            let _ = vec.extend_from_slice(data);
            vec
        })
}

/// Remove a bonded device
pub async fn remove_bonded_device(conn_handle: u16) -> Result<(), BondingError> {
    let mut storage = get_bonding_storage().lock().await;
    storage.remove_bonded_device(conn_handle)
}

/// Get the number of bonded devices
pub async fn bonded_device_count() -> usize {
    let storage = get_bonding_storage().lock().await;
    storage.device_count()
}

/// Check if a device is bonded
pub async fn is_device_bonded(conn_handle: u16) -> bool {
    let storage = get_bonding_storage().lock().await;
    storage.bonded_devices.contains_key(&conn_handle)
}

/// Get all bonded device handles (for testing/cleanup)
pub async fn get_all_bonded_handles() -> heapless::Vec<u16, MAX_BONDED_DEVICES> {
    let storage = get_bonding_storage().lock().await;
    let mut handles = heapless::Vec::new();
    for &handle in storage.bonded_devices.keys() {
        let _ = handles.push(handle);
    }
    handles
}

/// Get bonded device information (for testing)
pub async fn get_bonded_device_info(conn_handle: u16) -> Option<BondedDevice> {
    let storage = get_bonding_storage().lock().await;
    storage.bonded_devices.get(&conn_handle).cloned()
}
