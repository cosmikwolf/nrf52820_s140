//! Buffer Pool Management
//!
//! This module provides static buffer pools for TX and RX operations.
//! Uses atomic-pool for zero-allocation buffer management.

use atomic_pool::{pool, Box};
use defmt::Format;

/// Buffer size: BLE_EVT_LEN_MAX (247) + 2 bytes for response code
pub const BUFFER_SIZE: usize = 249;

/// Number of TX buffers (matches original C implementation)
pub const TX_POOL_SIZE: usize = 8;

// TX buffer pool - 8 buffers of 249 bytes each (doc comment not supported on macros)
pool!(TxPool: [[u8; BUFFER_SIZE]; TX_POOL_SIZE]);

/// RX buffer size for command reception
pub const RX_BUFFER_SIZE: usize = BUFFER_SIZE;

/// TX packet structure
pub struct TxPacket {
    data: Box<TxPool>,
    len: usize,
}

/// RX buffer for incoming commands
pub struct RxBuffer {
    data: [u8; RX_BUFFER_SIZE],
    len: usize,
}

/// Buffer pool errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum BufferError {
    /// No buffers available in pool
    PoolExhausted,
    /// Buffer too small for data
    BufferTooSmall,
    /// Invalid buffer size
    InvalidSize,
}

impl TxPacket {
    /// Allocate a new TX packet from the pool
    pub fn new(data: &[u8]) -> Result<Self, BufferError> {
        if data.len() > BUFFER_SIZE {
            return Err(BufferError::BufferTooSmall);
        }

        let mut buffer = Box::<TxPool>::new([0; BUFFER_SIZE]).ok_or(BufferError::PoolExhausted)?;

        buffer[..data.len()].copy_from_slice(data);

        Ok(Self {
            data: buffer,
            len: data.len(),
        })
    }

    /// Get the packet data as a slice
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    /// Get the packet length
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if packet is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl RxBuffer {
    /// Create a new RX buffer
    pub fn new() -> Self {
        Self {
            data: [0; RX_BUFFER_SIZE],
            len: 0,
        }
    }

    /// Get mutable slice for writing data
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Set the actual data length after receiving
    pub fn set_len(&mut self, len: usize) -> Result<(), BufferError> {
        if len > RX_BUFFER_SIZE {
            return Err(BufferError::InvalidSize);
        }
        self.len = len;
        Ok(())
    }

    /// Get the received data as a slice
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    /// Get the buffer length
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl Default for RxBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// TX queue for managing outgoing packets
pub struct TxQueue {
    queue: heapless::Deque<TxPacket, TX_POOL_SIZE>,
}

impl TxQueue {
    /// Create a new TX queue
    pub fn new() -> Self {
        Self {
            queue: heapless::Deque::new(),
        }
    }

    /// Enqueue a packet for transmission
    pub fn enqueue(&mut self, packet: TxPacket) -> Result<(), BufferError> {
        self.queue.push_back(packet).map_err(|_| BufferError::PoolExhausted)
    }

    /// Dequeue the next packet for transmission
    pub fn dequeue(&mut self) -> Option<TxPacket> {
        self.queue.pop_front()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Check if queue is full
    pub fn is_full(&self) -> bool {
        self.queue.is_full()
    }

    /// Get queue length
    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

impl Default for TxQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffer pool statistics
#[derive(Debug, Clone, Copy, Format)]
pub struct PoolStats {
    pub tx_allocated: usize,
    pub tx_available: usize,
    pub rx_active: bool,
}

/// Get current pool statistics
pub fn get_stats() -> PoolStats {
    // TODO: atomic_pool doesn't expose available() method directly
    // For now, we'll return placeholder values
    PoolStats {
        tx_allocated: 0,            // Would need to track this manually
        tx_available: TX_POOL_SIZE, // Assuming all available for now
        rx_active: true,            // RX buffer is statically allocated
    }
}

// Tests moved to external test files to avoid no_std conflicts

/// Initialize buffer pool
pub fn init() {
    defmt::info!("Buffer pool initialized");
}
