//! SPI Communication Layer
//! 
//! This module handles dual SPI communication:
//! - TX SPI (SPIM0 - Master): Device → Host communication
//! - RX SPI (SPIS1 - Slave): Host → Device communication

use defmt::{debug, error, info, warn, Format};
use embassy_nrf::{
    bind_interrupts,
    gpio::{Level, Output, OutputDrive},
    peripherals::{TWISPI0, TWISPI1, P0_00, P0_01, P0_02, P0_03, P0_04, P0_05, P0_06, P0_07},
    spim::{self, Spim, Frequency},
    spis::{self, Spis},
    Peri,
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::Channel,
};
use embassy_time::{Duration, Timer};

use crate::core::{
    memory::{TxPacket, BufferError},
    protocol::{Packet, ProtocolError},
};

bind_interrupts!(struct Irqs {
    TWISPI0 => spim::InterruptHandler<TWISPI0>;
    TWISPI1 => spis::InterruptHandler<TWISPI1>;
});

/// TX SPI Configuration (SPIM0 - Master)
/// Pins: SS=P0.01, SCK=P0.00, MOSI=P0.04, MISO=P0.02 (dummy)
/// Config: 8MHz, CPOL=High, CPHA=Leading, MSB First
pub struct TxSpiConfig {
    pub ss_pin: Peri<'static, P0_01>,
    pub sck_pin: Peri<'static, P0_00>,
    pub mosi_pin: Peri<'static, P0_04>,
    pub miso_pin: Peri<'static, P0_02>, // Dummy MISO for master mode
}

/// RX SPI Configuration (SPIS1 - Slave)  
/// Pins: SS=P0.07, SCK=P0.06, MOSI=P0.05, MISO=P0.03 (dummy)
/// Config: CPOL=High, CPHA=Leading, MSB First
pub struct RxSpiConfig {
    pub ss_pin: Peri<'static, P0_07>,
    pub sck_pin: Peri<'static, P0_06>,
    pub mosi_pin: Peri<'static, P0_05>,
    pub miso_pin: Peri<'static, P0_03>, // Dummy MISO for slave mode
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
pub static TX_CHANNEL: Channel<CriticalSectionRawMutex, TxPacket, 8> = Channel::new();

/// Channel for RX packets (from RX SPI task to command processor)
pub static RX_CHANNEL: Channel<CriticalSectionRawMutex, Packet, 1> = Channel::new();

/// TX SPI task - handles Device → Host communication
/// Receives packets from TX_CHANNEL and transmits them via SPIM0
#[embassy_executor::task]
pub async fn tx_spi_task(
    ss_pin: Peri<'static, P0_01>,
    sck_pin: Peri<'static, P0_00>,
    mosi_pin: Peri<'static, P0_04>,
    miso_pin: Peri<'static, P0_02>,
    spim0: Peri<'static, TWISPI0>,
) {
    info!("Starting TX SPI task (SPIM0 - Master)");
    
    // Configure SPI pins
    let mut ss = Output::new(ss_pin, Level::High, OutputDrive::Standard);
    
    let mut config = spim::Config::default();
    config.frequency = Frequency::M8;
    config.mode = spim::Mode {
        polarity: spim::Polarity::IdleHigh,
        phase: spim::Phase::CaptureOnSecondTransition,
    };
    
    let mut spi = Spim::new(spim0, Irqs, sck_pin, mosi_pin, miso_pin, config);
    
    info!("TX SPI configured: 8MHz, CPOL=High, CPHA=Leading");
    
    loop {
        // Wait for packet to transmit
        let tx_packet = TX_CHANNEL.receive().await;
        
        // Get serialized data
        let data = tx_packet.as_slice();
        
        debug!("TX SPI: Sending {} bytes", data.len());
        
        // Pull SS low to start transmission
        ss.set_low();
        Timer::after(Duration::from_micros(10)).await;
        
        // EasyDMA requires data in RAM - copy to local buffer
        let mut tx_buffer = [0u8; 256];
        let len = data.len().min(256);
        tx_buffer[..len].copy_from_slice(&data[..len]);
        
        let mut rx_buffer = [0u8; 256];
        let transfer_result = spi.transfer(&mut rx_buffer[..len], &tx_buffer[..len]).await;
        
        // Release SS
        Timer::after(Duration::from_micros(10)).await;
        ss.set_high();
        
        match transfer_result {
            Ok(_) => {
                debug!("TX SPI: Transfer completed successfully");
            },
            Err(e) => {
                error!("TX SPI: Transfer failed: {:?}", defmt::Debug2Format(&e));
            }
        }
        
        // Release packet buffer back to pool
        drop(tx_packet);
    }
}

/// RX SPI task - handles Host → Device communication  
/// Receives data via SPIS1 and forwards packets to RX_CHANNEL
#[embassy_executor::task]
pub async fn rx_spi_task(
    ss_pin: Peri<'static, P0_07>,
    sck_pin: Peri<'static, P0_06>,
    mosi_pin: Peri<'static, P0_05>,
    miso_pin: Peri<'static, P0_03>,
    spis1: Peri<'static, TWISPI1>,
) {
    info!("Starting RX SPI task (SPIS1 - Slave)");
    
    let mut config = spis::Config::default();
    config.mode = spis::Mode {
        polarity: spis::Polarity::IdleHigh,
        phase: spis::Phase::CaptureOnSecondTransition,
    };
    
    let mut spi = Spis::new(spis1, Irqs, sck_pin, ss_pin, mosi_pin, miso_pin, config);
    
    info!("RX SPI configured: Slave mode, CPOL=High, CPHA=Leading");
    
    loop {
        // Buffer for incoming data (EasyDMA requires RAM buffers)
        let mut rx_buffer = [0u8; 256];
        let tx_dummy = [0u8; 256];
        
        debug!("RX SPI: Waiting for host transmission...");
        
        // Wait for SPI transaction from host
        match spi.transfer(&mut rx_buffer, &tx_dummy).await {
            Ok((rx_len, _tx_len)) => {
                if rx_len > 0 {
                    debug!("RX SPI: Received {} bytes", rx_len);
                    
                    // Try to parse as protocol packet
                    match Packet::new_request(&rx_buffer[..rx_len]) {
                        Ok(packet) => {
                            debug!("RX SPI: Valid packet received, code: {:#04x}", packet.code);
                            
                            // Send to command processor
                            if RX_CHANNEL.try_send(packet).is_err() {
                                warn!("RX SPI: RX channel full, dropping packet");
                            }
                        },
                        Err(e) => {
                            warn!("RX SPI: Invalid packet received: {:?}", e);
                        }
                    }
                } else {
                    debug!("RX SPI: Empty transfer received");
                }
            },
            Err(e) => {
                error!("RX SPI: Transfer error: {:?}", defmt::Debug2Format(&e));
                Timer::after(Duration::from_millis(10)).await;
            }
        }
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

/// Initialize SPI communication and spawn tasks
pub async fn init_and_spawn(
    spawner: &embassy_executor::Spawner,
    tx_config: TxSpiConfig,
    rx_config: RxSpiConfig,
    spim0: Peri<'static, TWISPI0>,
    spis1: Peri<'static, TWISPI1>,
) -> Result<(), embassy_executor::SpawnError> {
    info!("Initializing SPI communication...");
    info!("TX SPI: SPIM0, 8MHz, pins SCK=P0.00, SS=P0.01, MOSI=P0.04");
    info!("RX SPI: SPIS1, slave mode, pins SCK=P0.06, SS=P0.07, MOSI=P0.05");
    
    // Spawn TX SPI task
    spawner.spawn(tx_spi_task(
        tx_config.ss_pin,
        tx_config.sck_pin,
        tx_config.mosi_pin,
        tx_config.miso_pin,
        spim0,
    ))?;
    
    // Spawn RX SPI task 
    spawner.spawn(rx_spi_task(
        rx_config.ss_pin,
        rx_config.sck_pin,
        rx_config.mosi_pin,
        rx_config.miso_pin,
        spis1,
    ))?;
    
    info!("SPI tasks spawned successfully");
    Ok(())
}

/// Legacy init function for backwards compatibility
pub fn init() {
    info!("SPI communication module initialized (legacy)");
}