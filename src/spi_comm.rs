//! SPI Communication Layer
//! 
//! This module handles dual SPI communication:
//! - TX SPI (SPIM0 - Master): Device → Host communication
//! - RX SPI (SPIS1 - Slave): Host → Device communication

use defmt::{debug, error, info, warn, Format};
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    peripherals::{P0_00, P0_01, P0_04, P0_05, P0_06, P0_07, TWISPI0, TWISPI1},
    spim::{self, Spim},
    spis::{self, Spis},
};
use embassy_sync::{
    blocking_mutex::raw::NoopRawMutex,
    channel::Channel,
};
use embassy_time::{Duration, Timer};

use crate::{
    buffer_pool::{TxPacket, RxBuffer, BufferError},
    protocol::{Packet, ProtocolError},
};

/// TX SPI Configuration (SPIM0 - Master)
/// Pins: SS=P0.01, SCK=P0.00, MOSI=P0.04, MISO=P0.02 (dummy)
/// Config: 8MHz, CPOL=High, CPHA=Leading, MSB First
pub struct TxSpiConfig {
    pub ss_pin: P0_01,
    pub sck_pin: P0_00,
    pub mosi_pin: P0_04,
    pub miso_pin: embassy_nrf::peripherals::P0_02, // Dummy MISO for master mode
}

/// RX SPI Configuration (SPIS1 - Slave)  
/// Pins: SS=P0.07, SCK=P0.06, MOSI=P0.05, MISO=P0.03 (dummy)
/// Config: CPOL=High, CPHA=Leading, MSB First
pub struct RxSpiConfig {
    pub ss_pin: P0_07,
    pub sck_pin: P0_06,
    pub mosi_pin: P0_05,
    pub miso_pin: embassy_nrf::peripherals::P0_03, // Dummy MISO for slave mode
}

/// SPI communication errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum SpiError {
    /// TX SPI transfer failed
    TxTransferFailed,
    /// RX SPI transfer failed
    RxTransferFailed,
    /// Buffer error
    BufferError(BufferError),
    /// Protocol error
    ProtocolError(ProtocolError),
    /// Timeout occurred
    Timeout,
    /// SPI not ready
    NotReady,
}

impl From<BufferError> for SpiError {
    fn from(err: BufferError) -> Self {
        SpiError::BufferError(err)
    }
}

impl From<ProtocolError> for SpiError {
    fn from(err: ProtocolError) -> Self {
        SpiError::ProtocolError(err)
    }
}

/// Channel for TX packets (from command processor to TX SPI task)
pub static TX_CHANNEL: Channel<NoopRawMutex, TxPacket, 8> = Channel::new();

/// Channel for RX packets (from RX SPI task to command processor)
pub static RX_CHANNEL: Channel<NoopRawMutex, Packet, 1> = Channel::new();

/// TX SPI task - handles Device → Host communication
#[embassy_executor::task]
pub async fn tx_spi_task(
    spim: TWISPI0,
    config: TxSpiConfig,
) {
    info!("Starting TX SPI task");

    // Configure SPI pins
    let mut spi_config = spim::Config::default();
    spi_config.frequency = spim::Frequency::M8; // 8 MHz
    spi_config.mode = spim::MODE_3; // CPOL=High, CPHA=Leading

    // Create SPI master instance
    let mut spi = Spim::new(
        spim,
        embassy_nrf::interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_TWISPI0),
        config.sck_pin,
        config.mosi_pin,
        config.miso_pin.degrade(), // Dummy MISO
        spi_config,
    );

    // Create SS (Chip Select) pin - active low
    let mut ss = Output::new(config.ss_pin, Level::High, OutputDrive::Standard);

    info!("TX SPI configured: 8MHz, CPOL=High, CPHA=Leading");

    loop {
        // Wait for packet to transmit
        let packet = TX_CHANNEL.receive().await;
        
        debug!("TX SPI: Sending packet of {} bytes", packet.len());

        // Assert SS (active low)
        ss.set_low();
        
        // Small delay for SS setup time
        Timer::after(Duration::from_micros(1)).await;

        // Transmit packet data
        match spi.write(packet.as_slice()).await {
            Ok(()) => {
                debug!("TX SPI: Packet transmitted successfully");
            }
            Err(e) => {
                error!("TX SPI: Transfer failed: {:?}", e);
            }
        }

        // Deassert SS
        ss.set_high();
        
        // Small delay before next transfer
        Timer::after(Duration::from_micros(10)).await;
    }
}

/// RX SPI task - handles Host → Device communication
#[embassy_executor::task]
pub async fn rx_spi_task(
    spis: TWISPI1,
    config: RxSpiConfig,
) {
    info!("Starting RX SPI task");

    // Configure SPI slave
    let spi_config = spis::Config::default();

    // Create SPI slave instance
    let mut spi = Spis::new(
        spis,
        embassy_nrf::interrupt::take!(SPIM1_SPIS1_TWIM1_TWIS1_TWISPI1),
        config.ss_pin,
        config.sck_pin,
        config.mosi_pin,
        config.miso_pin.degrade(), // Dummy MISO
        spi_config,
    );

    info!("RX SPI configured: Slave mode, CPOL=High, CPHA=Leading");

    loop {
        let mut rx_buffer = RxBuffer::new();
        
        debug!("RX SPI: Waiting for data from host");

        // Receive data from host
        match spi.read(rx_buffer.as_mut_slice()).await {
            Ok(bytes_received) => {
                if bytes_received > 0 {
                    debug!("RX SPI: Received {} bytes", bytes_received);
                    
                    if let Err(e) = rx_buffer.set_len(bytes_received) {
                        error!("RX SPI: Failed to set buffer length: {:?}", e);
                        continue;
                    }

                    // Parse packet
                    match Packet::new_request(rx_buffer.as_slice()) {
                        Ok(packet) => {
                            debug!("RX SPI: Parsed packet with code 0x{:04X}", packet.code);
                            
                            // Send to command processor
                            if RX_CHANNEL.try_send(packet).is_err() {
                                warn!("RX SPI: Command processor busy, dropping packet");
                            }
                        }
                        Err(e) => {
                            error!("RX SPI: Failed to parse packet: {:?}", e);
                        }
                    }
                } else {
                    debug!("RX SPI: No data received");
                }
            }
            Err(e) => {
                error!("RX SPI: Receive failed: {:?}", e);
                // Wait a bit before retrying
                Timer::after(Duration::from_millis(10)).await;
            }
        }
        
        // Small delay before next receive
        Timer::after(Duration::from_millis(1)).await;
    }
}

/// Send a response packet via TX SPI
pub async fn send_response(packet: TxPacket) -> Result<(), SpiError> {
    TX_CHANNEL.send(packet).await;
    Ok(())
}

/// Check if TX channel has space
pub fn tx_has_space() -> bool {
    !TX_CHANNEL.is_full()
}

/// Check if RX channel has data
pub fn rx_has_data() -> bool {
    !RX_CHANNEL.is_empty()
}

/// Try to receive a command packet (non-blocking)
pub fn try_receive_command() -> Option<Packet> {
    RX_CHANNEL.try_receive().ok()
}

/// Receive a command packet (blocking)
pub async fn receive_command() -> Packet {
    RX_CHANNEL.receive().await
}

/// Initialize SPI communication
pub fn init() {
    info!("SPI communication module initialized");
    info!("TX SPI: SPIM0, 8MHz, pins SCK=P0.00, SS=P0.01, MOSI=P0.04");
    info!("RX SPI: SPIS1, slave mode, pins SCK=P0.06, SS=P0.07, MOSI=P0.05");
}