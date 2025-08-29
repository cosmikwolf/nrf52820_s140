//! Bonding Service
//!
//! Manages BLE bonding and system attributes for persistent connections.
//! Handles CCCD states and other client-specific data.

use defmt::{debug, error, info, warn, Format};
use heapless::index_map::FnvIndexMap;

/// Maximum number of bonded devices
const MAX_BONDED_DEVICES: usize = 2;

/// Maximum system attributes data size
const MAX_SYS_ATTR_SIZE: usize = 64;

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
        Self {
            bonded_devices: FnvIndexMap::new(),
            next_bond_id: 1,
        }
    }

    fn add_bonded_device(&mut self, conn_handle: u16, peer_addr: [u8; 6], addr_type: u8) -> Result<(), BondingError> {
        let device = BondedDevice {
            conn_handle,
            peer_addr,
            addr_type,
            sys_attr_data: heapless::Vec::new(),
        };

        if self.bonded_devices.insert(conn_handle, device).is_err() {
            error!("BONDING: Failed to add bonded device - table full");
            return Err(BondingError::BondingTableFull);
        }

        debug!("BONDING: Added bonded device for connection {}", conn_handle);
        Ok(())
    }

    fn set_system_attributes(&mut self, conn_handle: u16, sys_attr_data: &[u8]) -> Result<(), BondingError> {
        match self.bonded_devices.get_mut(&conn_handle) {
            Some(device) => {
                device.sys_attr_data.clear();
                if device.sys_attr_data.extend_from_slice(sys_attr_data).is_err() {
                    error!(
                        "BONDING: System attributes data too large for connection {}",
                        conn_handle
                    );
                    return Err(BondingError::InvalidData);
                }
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
        self.bonded_devices.len()
    }
}

/// Global bonding storage
static mut BONDING_STORAGE: Option<BondingStorage> = None;

/// Initialize the bonding service
pub fn init() {
    unsafe {
        BONDING_STORAGE = Some(BondingStorage::new());
    }
    info!("BONDING: Bonding service initialized");
}

/// Add a bonded device
pub fn add_bonded_device(conn_handle: u16, peer_addr: [u8; 6], addr_type: u8) -> Result<(), BondingError> {
    cortex_m::interrupt::free(|_cs| unsafe {
        BONDING_STORAGE
            .as_mut()
            .unwrap()
            .add_bonded_device(conn_handle, peer_addr, addr_type)
    })
}

/// Set system attributes for a bonded device
pub fn set_system_attributes(conn_handle: u16, sys_attr_data: &[u8]) -> Result<(), BondingError> {
    cortex_m::interrupt::free(|_cs| unsafe {
        BONDING_STORAGE
            .as_mut()
            .unwrap()
            .set_system_attributes(conn_handle, sys_attr_data)
    })
}

/// Get system attributes for a bonded device
pub fn get_system_attributes(conn_handle: u16) -> Option<heapless::Vec<u8, MAX_SYS_ATTR_SIZE>> {
    cortex_m::interrupt::free(|_cs| unsafe {
        BONDING_STORAGE
            .as_ref()
            .unwrap()
            .get_system_attributes(conn_handle)
            .map(|data| {
                let mut vec = heapless::Vec::new();
                let _ = vec.extend_from_slice(data);
                vec
            })
    })
}

/// Remove a bonded device
pub fn remove_bonded_device(conn_handle: u16) -> Result<(), BondingError> {
    cortex_m::interrupt::free(|_cs| unsafe { BONDING_STORAGE.as_mut().unwrap().remove_bonded_device(conn_handle) })
}

/// Get the number of bonded devices
pub fn bonded_device_count() -> usize {
    cortex_m::interrupt::free(|_cs| unsafe { BONDING_STORAGE.as_ref().unwrap().device_count() })
}

/// Check if a device is bonded
pub fn is_device_bonded(conn_handle: u16) -> bool {
    cortex_m::interrupt::free(|_cs| unsafe {
        BONDING_STORAGE
            .as_ref()
            .unwrap()
            .bonded_devices
            .contains_key(&conn_handle)
    })
}
