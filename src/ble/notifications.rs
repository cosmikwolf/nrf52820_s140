//! Notification Service
//!
//! Manages BLE notifications and indications for the dynamic GATT system.
//! Provides a way to send notifications/indications via connection handles.

use defmt::{debug, error, info, warn, Format};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use heapless::Vec;

/// Maximum data length for notifications/indications
pub const MAX_NOTIFICATION_DATA: usize = 64;

/// Notification request
#[derive(Debug, Clone)]
pub struct NotificationRequest {
    pub conn_handle: u16,
    pub char_handle: u16,
    pub data: Vec<u8, MAX_NOTIFICATION_DATA>,
    pub is_indication: bool,
    pub response_id: u32,
}

/// Notification response
#[derive(Debug, Clone)]
pub struct NotificationResponse {
    pub response_id: u32,
    pub result: Result<(), NotificationError>,
}

/// Notification errors
#[derive(Debug, Clone, Copy, Format)]
pub enum NotificationError {
    ConnectionNotFound,
    CharacteristicNotFound,
    NotificationNotEnabled,
    DataTooLarge,
    SendFailed,
}

/// Channel for notification requests
static NOTIFICATION_CHANNEL: Channel<CriticalSectionRawMutex, NotificationRequest, 8> = Channel::new();

/// Channel for notification responses
static NOTIFICATION_RESPONSE_CHANNEL: Channel<CriticalSectionRawMutex, NotificationResponse, 8> = Channel::new();

/// Global request ID counter for notifications
static mut NOTIFICATION_REQUEST_ID: u32 = 1;

/// Send a notification to a specific connection
pub async fn send_notification(conn_handle: u16, char_handle: u16, data: &[u8]) -> Result<(), NotificationError> {
    let request_id = unsafe {
        let id = NOTIFICATION_REQUEST_ID;
        NOTIFICATION_REQUEST_ID = NOTIFICATION_REQUEST_ID.wrapping_add(1);
        id
    };

    // Validate data size
    if data.len() > MAX_NOTIFICATION_DATA {
        return Err(NotificationError::DataTooLarge);
    }

    let mut data_vec = Vec::new();
    if data_vec.extend_from_slice(data).is_err() {
        return Err(NotificationError::DataTooLarge);
    }

    let request = NotificationRequest {
        conn_handle,
        char_handle,
        data: data_vec,
        is_indication: false,
        response_id: request_id,
    };

    // Send the request
    NOTIFICATION_CHANNEL.send(request).await;

    // Wait for response
    loop {
        let response = NOTIFICATION_RESPONSE_CHANNEL.receive().await;
        if response.response_id == request_id {
            return response.result;
        }
        debug!(
            "Received notification response for different request ID: {}",
            response.response_id
        );
    }
}

/// Send an indication to a specific connection
pub async fn send_indication(conn_handle: u16, char_handle: u16, data: &[u8]) -> Result<(), NotificationError> {
    let request_id = unsafe {
        let id = NOTIFICATION_REQUEST_ID;
        NOTIFICATION_REQUEST_ID = NOTIFICATION_REQUEST_ID.wrapping_add(1);
        id
    };

    // Validate data size
    if data.len() > MAX_NOTIFICATION_DATA {
        return Err(NotificationError::DataTooLarge);
    }

    let mut data_vec = Vec::new();
    if data_vec.extend_from_slice(data).is_err() {
        return Err(NotificationError::DataTooLarge);
    }

    let request = NotificationRequest {
        conn_handle,
        char_handle,
        data: data_vec,
        is_indication: true,
        response_id: request_id,
    };

    // Send the request
    NOTIFICATION_CHANNEL.send(request).await;

    // Wait for response
    loop {
        let response = NOTIFICATION_RESPONSE_CHANNEL.receive().await;
        if response.response_id == request_id {
            return response.result;
        }
        debug!(
            "Received indication response for different request ID: {}",
            response.response_id
        );
    }
}

/// Notification service task that processes notification requests
///
/// This task needs to be spawned and will handle sending notifications
/// through the BLE stack when active connections are available.
#[embassy_executor::task]
pub async fn notification_service_task() {
    info!("Notification service task started");

    loop {
        let request = NOTIFICATION_CHANNEL.receive().await;
        debug!("Processing notification request: {:?}", request);

        let result = process_notification_request(&request).await;

        // Send response
        let response = NotificationResponse {
            response_id: request.response_id,
            result,
        };

        if let Err(_) = NOTIFICATION_RESPONSE_CHANNEL.try_send(response) {
            error!("Failed to send notification response - channel full");
        }
    }
}

/// Process a notification request
///
/// Currently returns placeholder results since we need to integrate with
/// the actual Connection objects from the advertising/connection tasks.
async fn process_notification_request(request: &NotificationRequest) -> Result<(), NotificationError> {
    debug!(
        "Processing {} for conn {} char {}",
        if request.is_indication {
            "indication"
        } else {
            "notification"
        },
        request.conn_handle,
        request.char_handle
    );

    // Check if connection exists
    let connection_exists =
        crate::ble::connection::with_connection_manager(|mgr| mgr.get_connection(request.conn_handle).is_some());

    if !connection_exists {
        warn!(
            "Attempted to send notification to unknown connection {}",
            request.conn_handle
        );
        return Err(NotificationError::ConnectionNotFound);
    }

    // For now, we can't actually send notifications since we don't have access
    // to the Connection objects. This would require architectural changes to
    // store Connection objects in a way that's accessible from here.

    // TODO: Actual implementation would:
    // 1. Get the Connection object for the handle
    // 2. Check if notifications/indications are enabled for the characteristic
    // 3. Send via nrf_softdevice::ble::gatt_server::notify/indicate

    warn!(
        "Notification sending not yet implemented - would send {} bytes to conn {} char {}",
        request.data.len(),
        request.conn_handle,
        request.char_handle
    );

    // Return success for now (placeholder)
    Ok(())
}
