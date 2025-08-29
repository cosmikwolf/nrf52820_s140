//! Service Manager for Dynamic GATT Service Creation
//!
//! This module provides a channel-based system for dynamic service creation
//! that respects nrf-softdevice's requirement for mutable Softdevice access.

use defmt::{debug, error, info, warn};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use heapless::index_map::FnvIndexMap;
use nrf_softdevice::ble::gatt_server::builder::ServiceBuilder;
use nrf_softdevice::ble::gatt_server::characteristic::{Attribute, Metadata, Properties};
use nrf_softdevice::ble::gatt_server::{CharacteristicHandles, RegisterError};
use nrf_softdevice::ble::Uuid;
use nrf_softdevice::Softdevice;

use crate::ble::registry::{BleUuid, ServiceType};

/// Service creation request
#[derive(Debug, Clone)]
pub struct ServiceCreateRequest {
    pub uuid: BleUuid,
    pub service_type: ServiceType,
    pub response_id: u32, // Unique ID to match response
}

/// Service creation response
#[derive(Debug, Clone)]
pub struct ServiceCreateResponse {
    pub response_id: u32,
    pub result: Result<u16, ServiceCreateError>,
}

/// Characteristic creation request
#[derive(Debug, Clone)]
pub struct CharacteristicCreateRequest {
    pub service_handle: u16,
    pub uuid: BleUuid,
    pub properties: u8,
    pub max_length: u16,
    pub permissions: u8,
    pub initial_value: heapless::Vec<u8, 64>,
    pub response_id: u32,
}

/// Characteristic handles info that implements Clone
#[derive(Debug, Clone)]
pub struct CharacteristicHandlesInfo {
    pub value_handle: u16,
    pub user_desc_handle: u16,
    pub cccd_handle: u16,
    pub sccd_handle: u16,
}

impl From<CharacteristicHandles> for CharacteristicHandlesInfo {
    fn from(handles: CharacteristicHandles) -> Self {
        Self {
            value_handle: handles.value_handle,
            user_desc_handle: handles.user_desc_handle,
            cccd_handle: handles.cccd_handle,
            sccd_handle: handles.sccd_handle,
        }
    }
}

/// Characteristic creation response
#[derive(Debug, Clone)]
pub struct CharacteristicCreateResponse {
    pub response_id: u32,
    pub result: Result<CharacteristicHandlesInfo, ServiceCreateError>,
}

/// Service creation errors
#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum ServiceCreateError {
    RegisterError,
    UuidConversionFailed,
    RegistryFull,
    ServiceNotFound,
    InvalidParameters,
}

impl From<RegisterError> for ServiceCreateError {
    fn from(_: RegisterError) -> Self {
        ServiceCreateError::RegisterError
    }
}

/// Channel for service creation requests
static SERVICE_CREATE_CHANNEL: Channel<CriticalSectionRawMutex, ServiceCreateRequest, 4> = Channel::new();

/// Channel for service creation responses  
static SERVICE_RESPONSE_CHANNEL: Channel<CriticalSectionRawMutex, ServiceCreateResponse, 4> = Channel::new();

/// Channel for characteristic creation requests
static CHARACTERISTIC_CREATE_CHANNEL: Channel<CriticalSectionRawMutex, CharacteristicCreateRequest, 8> = Channel::new();

/// Channel for characteristic creation responses
static CHARACTERISTIC_RESPONSE_CHANNEL: Channel<CriticalSectionRawMutex, CharacteristicCreateResponse, 8> =
    Channel::new();

/// Global request ID counter
static mut NEXT_REQUEST_ID: u32 = 1;

/// Request a service to be created
///
/// This function sends a service creation request to the service manager task
/// and waits for the response.
pub async fn request_service_creation(uuid: BleUuid, service_type: ServiceType) -> Result<u16, ServiceCreateError> {
    let request_id = unsafe {
        let id = NEXT_REQUEST_ID;
        NEXT_REQUEST_ID = NEXT_REQUEST_ID.wrapping_add(1);
        id
    };

    let request = ServiceCreateRequest {
        uuid,
        service_type,
        response_id: request_id,
    };

    // Send the request
    SERVICE_CREATE_CHANNEL.send(request).await;

    // Wait for response
    loop {
        let response = SERVICE_RESPONSE_CHANNEL.receive().await;
        if response.response_id == request_id {
            return response.result;
        }
        // If it's not our response, put it back or ignore
        // (In a more sophisticated system, we'd have per-request channels)
        debug!("Received response for different request ID: {}", response.response_id);
    }
}

/// Request a characteristic to be created
///
/// This function sends a characteristic creation request to the service manager task
/// and waits for the response.
pub async fn request_characteristic_creation(
    service_handle: u16,
    uuid: BleUuid,
    properties: u8,
    max_length: u16,
    permissions: u8,
    initial_value: &[u8],
) -> Result<CharacteristicHandlesInfo, ServiceCreateError> {
    let request_id = unsafe {
        let id = NEXT_REQUEST_ID;
        NEXT_REQUEST_ID = NEXT_REQUEST_ID.wrapping_add(1);
        id
    };

    // Convert initial_value to heapless::Vec
    let mut initial_vec = heapless::Vec::new();
    if initial_vec.extend_from_slice(initial_value).is_err() {
        return Err(ServiceCreateError::InvalidParameters);
    }

    let request = CharacteristicCreateRequest {
        service_handle,
        uuid,
        properties,
        max_length,
        permissions,
        initial_value: initial_vec,
        response_id: request_id,
    };

    // Send the request
    CHARACTERISTIC_CREATE_CHANNEL.send(request).await;

    // Wait for response
    loop {
        let response = CHARACTERISTIC_RESPONSE_CHANNEL.receive().await;
        if response.response_id == request_id {
            return response.result;
        }
        debug!("Received response for different request ID: {}", response.response_id);
    }
}

/// Service builder storage for maintaining builders across requests
struct ServiceBuilderStorage {
    /// Map of service handle -> ServiceBuilder
    builders: FnvIndexMap<u16, ServiceBuilder<'static>, 4>,
    /// Counter for generating unique service handles
    next_handle: u16,
}

impl ServiceBuilderStorage {
    fn new() -> Self {
        Self {
            builders: FnvIndexMap::new(),
            next_handle: 0x0010, // Start from a reasonable handle
        }
    }

    fn add_builder(&mut self, builder: ServiceBuilder<'static>) -> u16 {
        let handle = self.next_handle;
        self.next_handle += 1;

        if self.builders.insert(handle, builder).is_err() {
            warn!("ServiceBuilder storage full, handle allocation may conflict");
        }
        handle
    }

    fn get_builder_mut(&mut self, handle: u16) -> Option<&mut ServiceBuilder<'static>> {
        self.builders.get_mut(&handle)
    }

    fn remove_builder(&mut self, handle: u16) -> Option<ServiceBuilder<'static>> {
        self.builders.remove(&handle)
    }
}

/// Global service builder storage
static mut SERVICE_BUILDERS: Option<ServiceBuilderStorage> = None;

/// Service manager task that processes service creation requests
///
/// This task must be spawned in the main function and runs alongside
/// the main application loop. It processes service creation requests
/// when the Softdevice is not being used for other operations.
#[embassy_executor::task]
pub async fn service_manager_task(sd: &'static Softdevice) {
    info!("Service manager task started");

    // Initialize service builder storage
    unsafe {
        SERVICE_BUILDERS = Some(ServiceBuilderStorage::new());
    }

    loop {
        // For simplicity, prioritize service requests first, then characteristics
        if let Ok(service_request) = SERVICE_CREATE_CHANNEL.try_receive() {
            process_service_request(sd, service_request).await;
        } else if let Ok(char_request) = CHARACTERISTIC_CREATE_CHANNEL.try_receive() {
            process_characteristic_request(char_request).await;
        } else {
            // If no requests pending, wait for a service request (prioritized)
            // In a more sophisticated implementation, we'd use proper select
            let service_request = SERVICE_CREATE_CHANNEL.receive().await;
            process_service_request(sd, service_request).await;
        }
    }
}

/// Process a service creation request
async fn process_service_request(sd: &'static Softdevice, request: ServiceCreateRequest) {
    debug!("Processing service creation request: {:?}", request);

    // Convert BleUuid to nrf-softdevice Uuid
    let uuid = crate::ble::registry::with_registry(|registry| request.uuid.to_softdevice_uuid(registry));

    let result = match uuid {
        Some(uuid) => {
            // Create the service using ServiceBuilder
            create_service_with_builder(sd, uuid, request.service_type).await
        }
        None => Err(ServiceCreateError::UuidConversionFailed),
    };

    // Send response
    let response = ServiceCreateResponse {
        response_id: request.response_id,
        result,
    };

    // Try to send response (non-blocking)
    if let Err(_) = SERVICE_RESPONSE_CHANNEL.try_send(response) {
        error!("Failed to send service creation response - channel full");
    }
}

/// Process a characteristic creation request
async fn process_characteristic_request(request: CharacteristicCreateRequest) {
    debug!("Processing characteristic creation request: {:?}", request);

    // Convert BleUuid to nrf-softdevice Uuid
    let uuid = crate::ble::registry::with_registry(|registry| request.uuid.to_softdevice_uuid(registry));

    let result = match uuid {
        Some(uuid) => {
            add_characteristic_to_service(
                request.service_handle,
                uuid,
                request.properties,
                request.max_length,
                &request.initial_value,
            )
            .await
        }
        None => Err(ServiceCreateError::UuidConversionFailed),
    };

    // Send response
    let response = CharacteristicCreateResponse {
        response_id: request.response_id,
        result,
    };

    // Try to send response (non-blocking)
    if let Err(_) = CHARACTERISTIC_RESPONSE_CHANNEL.try_send(response) {
        error!("Failed to send characteristic creation response - channel full");
    }
}

/// Create a service using ServiceBuilder and store the builder
async fn create_service_with_builder(
    sd: &'static Softdevice,
    uuid: Uuid,
    _service_type: ServiceType,
) -> Result<u16, ServiceCreateError> {
    // This still requires solving the mutable Softdevice issue
    // For now, we'll create the builder but can't actually register it

    // SAFETY: This is a controlled context where we have exclusive access to the Softdevice
    // However, we still can't safely convert &Softdevice to &mut Softdevice
    // The proper solution requires architectural changes in nrf-softdevice or our main loop

    warn!("ServiceBuilder creation still requires mutable Softdevice access - using placeholder");

    // Generate a handle and store it for future characteristic additions
    let handle = unsafe { SERVICE_BUILDERS.as_mut().unwrap().next_handle };

    unsafe {
        SERVICE_BUILDERS.as_mut().unwrap().next_handle += 1;
    }

    // Store in registry
    match crate::ble::registry::with_registry(|registry| {
        registry.add_service(
            handle,
            crate::ble::registry::BleUuid::Uuid16(0x1801), // Placeholder Generic Attribute service
            _service_type,
        )
    }) {
        Ok(()) => {
            info!("Created placeholder service with handle {}", handle);
            Ok(handle)
        }
        Err(_) => Err(ServiceCreateError::RegistryFull),
    }
}

/// Add a characteristic to an existing service
async fn add_characteristic_to_service(
    service_handle: u16,
    uuid: Uuid,
    properties: u8,
    max_length: u16,
    initial_value: &[u8],
) -> Result<CharacteristicHandlesInfo, ServiceCreateError> {
    debug!("Adding characteristic to service {}", service_handle);

    // For now, return placeholder handles since we can't actually create characteristics
    // without solving the ServiceBuilder persistence issue

    warn!("Characteristic creation requires ServiceBuilder persistence - using placeholder");

    let handles = CharacteristicHandlesInfo {
        value_handle: service_handle + 1,
        user_desc_handle: 0, // Not used
        cccd_handle: if properties & 0x30 != 0 { service_handle + 2 } else { 0 },
        sccd_handle: 0, // Not used
    };

    info!(
        "Created placeholder characteristic with value handle {}",
        handles.value_handle
    );
    Ok(handles)
}
