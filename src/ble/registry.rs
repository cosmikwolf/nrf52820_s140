//! GATT Registry for Dynamic Service Management
//!
//! This module provides memory-optimized storage for dynamically created
//! GATT services and characteristics, designed for nRF52820 constraints.

use defmt::Format;
use heapless::Vec;
use nrf_softdevice::ble::Uuid;

/// Maximum number of services we can register
pub const MAX_SERVICES: usize = 8;

/// Maximum number of characteristics we can register  
pub const MAX_CHARACTERISTICS: usize = 32;

/// Maximum number of UUID bases we can register
pub const MAX_UUID_BASES: usize = 4;

/// Compact service representation - 8 bytes per service
#[derive(Clone, Copy, Debug, Format)]
pub struct ServiceInfo {
    pub handle: u16,
    pub uuid_type: u8,    // 0=16bit, 1=128bit, 2=vendor_specific
    pub uuid_data: u16,   // 16-bit UUID or index into UUID table
    pub service_type: u8, // Primary=1, Secondary=2
    pub _reserved: u8,    // Alignment padding
}

/// Compact characteristic representation - 16 bytes per characteristic
#[derive(Clone, Copy, Debug, Format)]
pub struct CharacteristicInfo {
    pub service_handle: u16,
    pub value_handle: u16,
    pub cccd_handle: u16, // 0 if not present
    pub sccd_handle: u16, // 0 if not present
    pub uuid_data: u16,   // Same format as ServiceInfo
    pub properties: u8,   // Packed properties
    pub uuid_type: u8,    // UUID type
    pub max_len: u16,     // Maximum value length
    pub current_len: u16, // Current value length
    pub permissions: u8,  // Read/write permissions
    pub _reserved: u8,    // Alignment padding
}

/// Service type enumeration
#[derive(Clone, Copy, Debug, Format, PartialEq)]
#[repr(u8)]
pub enum ServiceType {
    Primary = 1,
    Secondary = 2,
}

/// UUID type enumeration
#[derive(Clone, Copy, Debug, Format, PartialEq)]
#[repr(u8)]
pub enum UuidType {
    Uuid16 = 0,
    Uuid128 = 1,
    VendorSpecific = 2,
}

/// Characteristic properties (matches BLE specification)
pub mod char_properties {
    pub const BROADCAST: u8 = 0x01;
    pub const READ: u8 = 0x02;
    pub const WRITE_WITHOUT_RESPONSE: u8 = 0x04;
    pub const WRITE: u8 = 0x08;
    pub const NOTIFY: u8 = 0x10;
    pub const INDICATE: u8 = 0x20;
    pub const AUTH_SIGNED_WRITES: u8 = 0x40;
    pub const EXTENDED_PROPERTIES: u8 = 0x80;
}

/// Registry errors
#[derive(Debug, Clone, Copy, Format, PartialEq)]
pub enum RegistryError {
    ServicesFull,
    CharacteristicsFull,
    UuidBasesFull,
    ServiceNotFound,
    CharacteristicNotFound,
    InvalidUuidType,
    InvalidServiceType,
}

/// Memory-constrained GATT registry
/// Total size: ~768 bytes (well within 1KB budget)
pub struct GattRegistry {
    // Service storage: 8 * 8 = 64 bytes
    services: [ServiceInfo; MAX_SERVICES],
    service_count: u8,

    // Characteristic storage: 32 * 16 = 512 bytes
    characteristics: [CharacteristicInfo; MAX_CHARACTERISTICS],
    characteristic_count: u8,

    // UUID base storage: 4 * 16 = 64 bytes
    uuid_bases: [[u8; 16]; MAX_UUID_BASES],
    uuid_base_count: u8,

    // Handle tracking
    next_service_id: u16,
    next_characteristic_id: u16,
}

impl GattRegistry {
    /// Create a new GATT registry
    pub const fn new() -> Self {
        Self {
            services: [ServiceInfo {
                handle: 0,
                uuid_type: 0,
                uuid_data: 0,
                service_type: 0,
                _reserved: 0,
            }; MAX_SERVICES],
            service_count: 0,

            characteristics: [CharacteristicInfo {
                service_handle: 0,
                value_handle: 0,
                cccd_handle: 0,
                sccd_handle: 0,
                uuid_data: 0,
                properties: 0,
                uuid_type: 0,
                max_len: 0,
                current_len: 0,
                permissions: 0,
                _reserved: 0,
            }; MAX_CHARACTERISTICS],
            characteristic_count: 0,

            uuid_bases: [[0; 16]; MAX_UUID_BASES],
            uuid_base_count: 0,

            next_service_id: 1,
            next_characteristic_id: 1,
        }
    }

    /// Register a UUID base and return its handle
    pub fn register_uuid_base(&mut self, uuid_base: [u8; 16]) -> Result<u8, RegistryError> {
        if self.uuid_base_count >= MAX_UUID_BASES as u8 {
            return Err(RegistryError::UuidBasesFull);
        }

        let handle = self.uuid_base_count;
        self.uuid_bases[handle as usize] = uuid_base;
        self.uuid_base_count += 1;

        Ok(handle)
    }

    /// Get UUID base by handle
    pub fn get_uuid_base(&self, handle: u8) -> Option<&[u8; 16]> {
        if handle < self.uuid_base_count {
            Some(&self.uuid_bases[handle as usize])
        } else {
            None
        }
    }

    /// Add a service to the registry
    pub fn add_service(&mut self, handle: u16, uuid: BleUuid, service_type: ServiceType) -> Result<(), RegistryError> {
        if self.service_count >= MAX_SERVICES as u8 {
            return Err(RegistryError::ServicesFull);
        }

        let (uuid_type, uuid_data) = match uuid {
            BleUuid::Uuid16(uuid) => (UuidType::Uuid16 as u8, uuid),
            BleUuid::Uuid128(_) => (UuidType::Uuid128 as u8, 0), // Store index separately
            BleUuid::VendorSpecific { base_id, offset } => (UuidType::VendorSpecific as u8, offset),
        };

        let service_info = ServiceInfo {
            handle,
            uuid_type,
            uuid_data,
            service_type: service_type as u8,
            _reserved: 0,
        };

        self.services[self.service_count as usize] = service_info;
        self.service_count += 1;
        self.next_service_id += 1;

        Ok(())
    }

    /// Add a characteristic to the registry
    pub fn add_characteristic(
        &mut self,
        service_handle: u16,
        value_handle: u16,
        cccd_handle: u16,
        sccd_handle: u16,
        uuid: BleUuid,
        properties: u8,
        max_len: u16,
        permissions: u8,
    ) -> Result<(), RegistryError> {
        if self.characteristic_count >= MAX_CHARACTERISTICS as u8 {
            return Err(RegistryError::CharacteristicsFull);
        }

        let (uuid_type, uuid_data) = match uuid {
            BleUuid::Uuid16(uuid) => (UuidType::Uuid16 as u8, uuid),
            BleUuid::Uuid128(_) => (UuidType::Uuid128 as u8, 0), // Store index separately
            BleUuid::VendorSpecific { base_id: _, offset } => (UuidType::VendorSpecific as u8, offset),
        };

        let char_info = CharacteristicInfo {
            service_handle,
            value_handle,
            cccd_handle,
            sccd_handle,
            uuid_data,
            properties,
            uuid_type,
            max_len,
            current_len: 0,
            permissions,
            _reserved: 0,
        };

        self.characteristics[self.characteristic_count as usize] = char_info;
        self.characteristic_count += 1;
        self.next_characteristic_id += 1;

        Ok(())
    }

    /// Find service by handle
    pub fn find_service(&self, handle: u16) -> Option<&ServiceInfo> {
        self.services[..self.service_count as usize]
            .iter()
            .find(|s| s.handle == handle)
    }

    /// Find characteristic by value handle
    pub fn find_characteristic_by_value_handle(&self, handle: u16) -> Option<&CharacteristicInfo> {
        self.characteristics[..self.characteristic_count as usize]
            .iter()
            .find(|c| c.value_handle == handle)
    }

    /// Find characteristic by CCCD handle
    pub fn find_characteristic_by_cccd_handle(&self, handle: u16) -> Option<&CharacteristicInfo> {
        self.characteristics[..self.characteristic_count as usize]
            .iter()
            .find(|c| c.cccd_handle == handle && c.cccd_handle != 0)
    }

    /// Get all services
    pub fn services(&self) -> &[ServiceInfo] {
        &self.services[..self.service_count as usize]
    }

    /// Get all characteristics
    pub fn characteristics(&self) -> &[CharacteristicInfo] {
        &self.characteristics[..self.characteristic_count as usize]
    }

    /// Get registry statistics
    pub fn stats(&self) -> (u8, u8, u8) {
        (self.service_count, self.characteristic_count, self.uuid_base_count)
    }

    /// Clear all entries (for testing)
    pub fn clear(&mut self) {
        self.service_count = 0;
        self.characteristic_count = 0;
        self.uuid_base_count = 0;
        self.next_service_id = 1;
        self.next_characteristic_id = 1;
    }
}

/// BLE UUID representation
#[derive(Debug, Clone, Copy, Format)]
pub enum BleUuid {
    Uuid16(u16),
    Uuid128([u8; 16]),
    VendorSpecific { base_id: u8, offset: u16 },
}

impl BleUuid {
    /// Convert to nrf-softdevice Uuid
    pub fn to_softdevice_uuid(&self, registry: &GattRegistry) -> Option<Uuid> {
        match self {
            BleUuid::Uuid16(uuid) => Some(Uuid::new_16(*uuid)),
            BleUuid::Uuid128(uuid) => Some(Uuid::new_128(uuid)),
            BleUuid::VendorSpecific { base_id, offset } => {
                if let Some(base) = registry.get_uuid_base(*base_id) {
                    let mut uuid = *base;
                    // Insert offset at bytes 12-13 (little-endian)
                    let offset_bytes = offset.to_le_bytes();
                    uuid[12] = offset_bytes[0];
                    uuid[13] = offset_bytes[1];
                    Some(Uuid::new_128(&uuid))
                } else {
                    None
                }
            }
        }
    }

    /// Parse UUID from payload data
    pub fn from_payload(uuid_type: u8, uuid_data: &[u8]) -> Result<Self, RegistryError> {
        match uuid_type {
            0 => {
                // 16-bit UUID
                if uuid_data.len() < 2 {
                    return Err(RegistryError::InvalidUuidType);
                }
                let uuid = u16::from_le_bytes([uuid_data[0], uuid_data[1]]);
                Ok(BleUuid::Uuid16(uuid))
            }
            1 => {
                // 128-bit UUID
                if uuid_data.len() < 16 {
                    return Err(RegistryError::InvalidUuidType);
                }
                let mut uuid = [0u8; 16];
                uuid.copy_from_slice(&uuid_data[..16]);
                Ok(BleUuid::Uuid128(uuid))
            }
            2 => {
                // Vendor-specific UUID (base_id + offset)
                if uuid_data.len() < 3 {
                    return Err(RegistryError::InvalidUuidType);
                }
                let base_id = uuid_data[0];
                let offset = u16::from_le_bytes([uuid_data[1], uuid_data[2]]);
                Ok(BleUuid::VendorSpecific { base_id, offset })
            }
            _ => Err(RegistryError::InvalidUuidType),
        }
    }
}

/// Global GATT registry instance
static mut GATT_REGISTRY: GattRegistry = GattRegistry::new();

/// Access the global GATT registry
///
/// # Safety
/// This function provides mutable access to a static registry.
/// It should only be called from a single thread (main async task).
pub fn with_registry<T, F>(f: F) -> T
where
    F: FnOnce(&mut GattRegistry) -> T,
{
    unsafe { f(&mut GATT_REGISTRY) }
}
