# Host SPI Implementation Analysis

**Date**: 2025-01-29  
**Source**: `/Users/tenkai/Development/RedSystemVentures/nrf52820_s140/.temp/ble_host_fw_c/`  
**Status**: Complete Analysis  

## Executive Summary

The host-side BLE firmware implements a sophisticated dual SPI communication system using STM32H7 hardware with DMA-based data transfer. The implementation features separate TX (master) and RX (slave) SPI interfaces with interrupt-driven packet handling and FreeRTOS integration for robust real-time communication with the nRF52820 BLE modem.

## Architecture Overview

### Dual SPI Configuration

**RX SPI (SPI1 - Slave Mode)**: Host ← nRF52820  
- **Purpose**: Receives responses and events from BLE modem
- **Mode**: Slave (modem controls clock)
- **DMA**: DMA1 Stream2 for automatic reception
- **Interrupt**: External interrupt on SS pin (PG10) for packet detection

**TX SPI (SPI2 - Master Mode)**: Host → nRF52820  
- **Purpose**: Sends commands to BLE modem  
- **Mode**: Master (host controls clock)
- **DMA**: DMA1 Stream1 for automatic transmission
- **Interrupt**: SPI2 End-of-Transfer interrupt for completion

## Hardware Configuration Analysis

### Pin Mappings (from `ble_comm_spi.cpp:109-122`)

#### RX SPI1 (Slave) Pins:
```c
GPIO_AF(GPIOG, 10, GPIO_AF_5);   // SS (Slave Select) - PG10
GPIO_AF(GPIOG, 11, GPIO_AF_5);   // SCK (Clock) - PG11  
GPIO_AF(GPIOD, 7, GPIO_AF_5);    // MOSI (Data In) - PD7
// Pin configurations:
GPIO_Init(GPIOG, 10, GPIO_Mode_AF, GPIO_Speed_Medium, GPIO_OType_OD, GPIO_PuPd_UP);    // SS with pull-up
GPIO_Init(GPIOG, 11, GPIO_Mode_AF, GPIO_Speed_Medium, GPIO_OType_OD, GPIO_PuPd_NOPULL); // SCK  
GPIO_Init(GPIOD, 7, GPIO_Mode_AF, GPIO_Speed_Medium, GPIO_OType_OD, GPIO_PuPd_NOPULL);  // MOSI
```

#### TX SPI2 (Master) Pins:
```c
GPIO_AF(GPIOB, 13, GPIO_AF_5);   // SCK (Clock) - PB13
GPIO_AF(GPIOB, 15, GPIO_AF_5);   // MOSI (Data Out) - PB15
GPIO_Init(GPIOB, 12, GPIO_Mode_OUT, GPIO_Speed_Medium, GPIO_OType_PP, GPIO_PuPd_NOPULL); // SS - PB12 (GPIO)
GPIO_Init(GPIOB, 13, GPIO_Mode_AF, GPIO_Speed_Medium, GPIO_OType_PP, GPIO_PuPd_NOPULL);   // SCK
GPIO_Init(GPIOB, 15, GPIO_Mode_AF, GPIO_Speed_Medium, GPIO_OType_PP, GPIO_PuPd_NOPULL);   // MOSI
```

### SPI Configuration Details

#### RX SPI1 Configuration (lines 133-142):
```c
REG_SR(SPI1->CFG1,
    // Data size: 8 bits
    (((8 - 1) << SPI_CFG1_DSIZE_Pos) & SPI_CFG1_DSIZE_Msk) | 
    // FIFO threshold: 1 byte 
    (((1 - 1) << SPI_CFG1_FTHLV_Pos) & SPI_CFG1_FTHLV_Msk) | 
    // Enable RX DMA
    SPI_CFG1_RXDMAEN,
    // Clear data size and threshold masks
    SPI_CFG1_DSIZE_Msk | SPI_CFG1_FTHLV_Msk
);
SPI1->CFG2 |= (2 << SPI_CFG2_COMM_Pos);  // Slave mode configuration
```

#### TX SPI2 Configuration (lines 185-202):
```c
REG_SR(SPI2->CFG1,
    // Data size: 8 bits
    (((8 - 1) << SPI_CFG1_DSIZE_Pos) & SPI_CFG1_DSIZE_Msk) | 
    // Clock divider: /16 (lines 188)
    (((4 - 1) << SPI_CFG1_MBR_Pos) & SPI_CFG1_MBR_Msk) | 
    // FIFO threshold: 1 byte
    (((1 - 1) << SPI_CFG1_FTHLV_Pos) & SPI_CFG1_FTHLV_Msk) | 
    // Enable TX DMA
    SPI_CFG1_TXDMAEN,
    // Clear relevant masks and disable CRC
    SPI_CFG1_DSIZE_Msk | SPI_CFG1_MBR_Msk | SPI_CFG1_FTHLV_Msk | SPI_CFG1_CRCEN
);

REG_SR(SPI2->CFG2,
    // Auto frame control, Master mode, SS output enable, Transmit only
    SPI_CFG2_AFCNTR | SPI_CFG2_MASTER | SPI_CFG2_SSOE | (1 << SPI_CFG2_COMM_Pos),
    // Clear polarity and phase settings (CPOL=0, CPHA=0)
    SPI_CFG2_LSBFRST | SPI_CFG2_CPOL | SPI_CFG2_CPHA | SPI_CFG2_SP | SPI_CFG2_COMM | SPI_CFG2_MIDI | SPI_CFG2_MSSI | 
    SPI_CFG2_IOSWP | SPI_CFG2_SSIOP | SPI_CFG2_SSOM
);
```

## Buffer Management System

### Buffer Pool Constants (from `ble_comm_spi.h:11-12`):
```c
#define COMM_PAYLOAD_SIZE   (BLE_EVT_LEN_MAX(NRF_SDH_BLE_GATT_MAX_MTU_SIZE) + 2)
#define COMM_PBUF_COUNT     16
```
Where `NRF_SDH_BLE_GATT_MAX_MTU_SIZE = 247` (from `ble_modem.h:18`)  
Therefore: `COMM_PAYLOAD_SIZE = BLE_EVT_LEN_MAX(247) + 2`

Where `BLE_EVT_LEN_MAX(ATT_MTU)` is defined as:
```c
#define BLE_EVT_LEN_MAX(ATT_MTU) ( \
    offsetof(ble_evt_t, evt.gattc_evt.params.prim_srvc_disc_rsp.services) + ((ATT_MTU) - 1) / 4 * sizeof(ble_gattc_service_t) \
)
```

This evaluates to a variable size based on the maximum GATT service discovery response.

### Memory Pool Architecture (lines 14-20):
```c
// TX Pool: 2 buffers for command transmission
__attribute__ ((section(".sram2")))
STATIC_PBUF_DECLARE_POOL(l_tx_pbuf_pool, 2, COMM_PAYLOAD_SIZE);

// RX Pool: 16 buffers for response/event reception  
__attribute__ ((section(".sram2")))
STATIC_PBUF_DECLARE_POOL(l_rx_pbuf_pool, COMM_PBUF_COUNT, COMM_PAYLOAD_SIZE);
```

**Memory Layout**:
- **TX Pool**: 2 × COMM_PAYLOAD_SIZE (stored in SRAM2)
- **RX Pool**: 16 × COMM_PAYLOAD_SIZE (stored in SRAM2) 
- **Total SPI Buffer Memory**: 18 × COMM_PAYLOAD_SIZE dedicated to SPI communication

### Buffer Lifecycle Management

**RX Buffer Flow**:
1. **Initialization**: Pre-allocate 16 RX buffers from pool
2. **Active Reception**: One buffer active for DMA reception  
3. **Interrupt Processing**: On packet complete, swap active buffer
4. **Queue Handoff**: Completed packets queued for processing thread
5. **Release & Recycle**: Processed packets returned to pool via callback

**TX Buffer Flow**:  
1. **Request Allocation**: Application requests TX buffer for command
2. **Packet Building**: Command data filled into buffer
3. **Transmission**: DMA transfers buffer to SPI2
4. **Automatic Release**: Buffer returned to pool on transfer complete

## DMA Configuration Analysis

### RX DMA (DMA1 Stream2) Setup (lines 145-158):
```c
// DMAMUX routing: SPI1 RX request (ID 37)
DMAMUX1_Channel2->CCR = (37 << DMAMUX_CxCR_DMAREQ_ID_Pos);

// DMA stream configuration
DMA1_Stream2->PAR = (uint32_t)&SPI1->RXDR;  // Peripheral address: SPI1 RX data register

// FIFO configuration  
REG_SR(DMA1_Stream2->FCR,
    DMA_SxFCR_DMDIS |           // Disable direct mode (use FIFO)
    (1 << DMA_SxFCR_FTH_Pos),   // FIFO threshold level 1
    DMA_SxFCR_FTH_Msk
);

// Runtime DMA setup (lines 28-32):
DMA1_Stream2->M0AR = (uint32_t)p->payload;  // Memory address: buffer payload
DMA1_Stream2->NDTR = p->capacity;           // Transfer count: buffer capacity
DMA1_Stream2->CR = (3 << DMA_SxCR_PL_Pos) |  // Highest priority
                   DMA_SxCR_MINC |            // Memory increment
                   DMA_SxCR_EN;               // Enable DMA
```

### TX DMA (DMA1 Stream1) Setup (lines 204-216):
```c
// DMAMUX routing: SPI2 TX request (ID 40)  
DMAMUX1_Channel1->CCR = (40 << DMAMUX_CxCR_DMAREQ_ID_Pos);

// DMA stream configuration
DMA1_Stream1->PAR = (uint32_t)&SPI2->TXDR;  // Peripheral address: SPI2 TX data register

// FIFO configuration
REG_SR(DMA1_Stream1->FCR,
    DMA_SxFCR_DMDIS |           // Disable direct mode (use FIFO)
    (1 << DMA_SxFCR_FTH_Pos),   // FIFO threshold level 1  
    DMA_SxFCR_FTH_Msk
);

// Runtime DMA setup (lines 282-287):
DMA1_Stream1->M0AR = (uint32_t)p->payload;  // Memory address: packet payload
DMA1_Stream1->NDTR = p->length;             // Transfer count: packet length
DMA1_Stream1->CR = DMA_SxCR_MINC |          // Memory increment
                   DMA_SxCR_DIR_0 |         // Memory to peripheral direction  
                   DMA_SxCR_EN;             // Enable DMA (lowest priority - default)
```

## Interrupt Handling Architecture

### RX Interrupt System

**External Interrupt Setup** (lines 161-167):
```c
// Configure EXTI10 (PG10 - SS pin) for rising edge
SYSCFG->EXTICR[2] = (SYSCFG->EXTICR[2] & ~SYSCFG_EXTICR3_EXTI10) | SYSCFG_EXTICR3_EXTI10_PG;
EXTI->FTSR1 &= ~EXTI_FTSR1_TR10;   // Disable falling edge trigger  
EXTI->RTSR1 |= EXTI_RTSR1_TR10;    // Enable rising edge trigger
EXTI_D1->PR1 = EXTI_PR1_PR10;      // Clear pending interrupt
EXTI_D1->IMR1 |= EXTI_IMR1_IM10;   // Enable interrupt mask
```

**Interrupt Handler** (`irq_handler` function, lines 37-81):
```c
static void irq_handler(BaseType_t* woken)
{
    // 1. Stop SPI and DMA immediately
    SPI1->CR1 &= ~SPI_CR1_SPE;
    DMA1_Stream2->CR = 0;
    while(DMA1_Stream2->CR & DMA_SxCR_EN);  // Wait for DMA to stop
    
    // 2. Clear all DMA flags
    DMA1->LIFCR = DMA_LIFCR_CFEIF2 | DMA_LIFCR_CDMEIF2 | DMA_LIFCR_CTEIF2 | DMA_LIFCR_CHTIF2 | DMA_LIFCR_CTCIF2;
    
    // 3. Calculate actual received bytes
    const uint32_t dma_size = DMA1_Stream2->NDTR;  // Remaining transfer count
    
    // 4. Allocate new buffer for next reception
    pbuf_t* const p = pbuf_acquire(&l_rx_pbuf_pool);
    if (p) {
        start_rx_dma(p);
        ++ble_internals_intrs_success;
    } else {
        EXTI->IMR1 &= ~EXTI_IMR1_IM10;  // Mask interrupt if no buffers
        ++ble_internals_intrs_failure;
    }
    
    // 5. Process completed packet
    if (l_rx_pbuf) {
        l_rx_pbuf->length = l_rx_pbuf->capacity - dma_size;  // Calculate actual length
        ble_internals_received += (uint32_t)l_rx_pbuf->length;
        ble_modem_process_packet(l_rx_pbuf, woken);  // Queue for processing
    }
    
    l_rx_pbuf = p;  // Set new active buffer
}
```

### TX Interrupt System

**SPI2 Interrupt Handler** (`SPI2_ISR`, lines 249-262):
```c
extern "C" void SPI2_ISR(void)
{
    // 1. Disable end-of-transfer interrupt
    SPI2->IER &= ~SPI_IER_EOTIE;
    SPI2->IFCR = SPI_IFCR_EOTC;
    
    // 2. Stop SPI
    SPI2->CR1 &= ~SPI_CR1_SPE;
    
    // 3. Stop and clean up DMA  
    DMA1_Stream1->CR = 0;
    DMA1->LIFCR = DMA_LIFCR_CFEIF1 | DMA_LIFCR_CDMEIF1 | DMA_LIFCR_CTEIF1 | DMA_LIFCR_CHTIF1 | DMA_LIFCR_CTCIF1;
    
    // 4. Release transmitted buffer
    pbuf_release(l_tx_pbuf);
    
    // 5. Deassert SS (set high)
    GPIO_SetBits(GPIOB, GPIO_Pin_12);
}
```

## Transmission Protocol Implementation

### Command Transmission Sequence (`ble_comm_send_request`, lines 265-298):

```c
void ble_comm_send_request(uint16_t request, pbuf_t* p)
{
    // 1. Prepare hardware
    DMA1_Stream1->CR = 0;                    // Stop any ongoing DMA
    GPIO_ResetBits(GPIOB, GPIO_Pin_12);      // Assert SS (pull low)
    
    // 2. Append request code to packet (big-endian format)
    p->capacity += 2;
    pbuf_insert_uint16(p, request);          // Add request code at end
    pbuf_reset(p);                           // Reset payload pointer to start
    
    // 3. Update statistics and store reference
    l_tx_pbuf = p;
    ble_internals_transmitted += (uint32_t)p->length;
    
    // 4. Configure DMA transfer
    while(DMA1_Stream1->CR & DMA_SxCR_EN);   // Ensure DMA stopped
    DMA1->LIFCR = DMA_LIFCR_CFEIF1 | DMA_LIFCR_CDMEIF1 | DMA_LIFCR_CTEIF1 | DMA_LIFCR_CHTIF1 | DMA_LIFCR_CTCIF1;
    
    __DMB();                                 // Memory barrier
    DMA1_Stream1->M0AR = (uint32_t)p->payload;  // Set memory address
    DMA1_Stream1->NDTR = p->length;             // Set transfer count
    DMA1_Stream1->CR = DMA_SxCR_MINC |          // Memory increment  
                       DMA_SxCR_DIR_0 |         // Memory to peripheral
                       DMA_SxCR_EN;             // Enable DMA
    
    // 5. Configure and start SPI transfer
    SPI2->CR2 = p->length;                   // Set transfer size
    SPI2->IFCR = SPI_IFCR_TXTFC;            // Clear TX FIFO
    SPI2->CR1 |= SPI_CR1_SPE;               // Enable SPI
    
    // 6. Enable completion interrupt and start transfer
    SPI2->IFCR = SPI_IFCR_EOTC;             // Clear end-of-transfer flag
    SPI2->IER |= SPI_IER_EOTIE;             // Enable end-of-transfer interrupt  
    SPI2->CR1 |= SPI_CR1_CSTART;           // Start transfer
}
```

## Error Handling & Recovery

### Buffer Exhaustion Handling

**RX Buffer Recovery** (`free_rx_cb`, lines 84-95):
```c
static void free_rx_cb(void* user)
{
    // Called when RX buffer is released back to pool
    if (l_rx_pbuf == NULL) {
        // If no active RX buffer, restart reception
        l_rx_pbuf = pbuf_acquire(&l_rx_pbuf_pool);
        start_rx_dma(l_rx_pbuf);
        
        // Re-enable external interrupt
        EXTI_D1->PR1 = EXTI_PR1_PR10;      // Clear pending
        EXTI_D1->IMR1 |= EXTI_IMR1_IM10;   // Unmask interrupt
    }
}
```

### Statistics & Monitoring

**Global Statistics Variables** (lines 122-126):
```c
uint16_t ble_internals_intrs_success = 0;   // Successful RX interrupts
uint16_t ble_internals_intrs_failure = 0;   // Failed RX interrupts (no buffers)
uint32_t ble_internals_transmitted = 0;     // Total bytes transmitted
uint32_t ble_internals_received = 0;        // Total bytes received
```

## Performance Characteristics

### Timing Parameters

**Clock Configuration**:
- **TX SPI2 Clock Divider**: /16 (`(((4 - 1) << SPI_CFG1_MBR_Pos)` line 188)
- **DMA Priority**: RX DMA has highest priority (3), TX DMA has lowest (0)
- **NVIC Priority**: `NVIC_PRIORITY_BLE_SPI` for SPI2 (line 218)

**Buffer Allocation**:
- **TX Timeout**: 1000ms (`pdMS_TO_TICKS(1000)` in modem implementation)
- **Memory Sections**: All buffers allocated in SRAM2 for optimal performance

### Throughput Considerations

**Maximum Packet Size**: 249 bytes (MTU + overhead)  
**Buffer Depth**: 16 RX buffers provide substantial buffering  
**DMA Efficiency**: Zero-copy transfers minimize CPU overhead  
**Interrupt Latency**: High-priority DMA and optimized ISR for real-time performance

## Integration Points

### FreeRTOS Integration

**Critical Sections**: TX buffer allocation uses `taskENTER_CRITICAL()`/`taskEXIT_CRITICAL()`  
**ISR Context**: RX handler operates in interrupt context with `BaseType_t* woken` parameter  
**Queue Integration**: Seamless handoff to modem processing thread via FreeRTOS queues

### Hardware Dependencies  

**STM32H7 Specific**: Uses advanced SPI features (auto frame control, DMAMUX, etc.)  
**Memory Architecture**: SRAM2 section usage suggests specific STM32H7 memory layout  
**Clock System**: Assumes specific APB1/APB2 clock configuration

## Summary

The host SPI implementation represents a sophisticated dual-channel communication system optimized for real-time BLE modem operation. Key strengths include:

- **Hardware-accelerated DMA** for zero-copy transfers
- **Interrupt-driven architecture** for minimal latency
- **Robust buffer management** with automatic recovery
- **Comprehensive error handling** and statistics
- **FreeRTOS integration** for multi-threaded operation

The implementation successfully abstracts the complexity of dual SPI communication while providing the high performance and reliability required for BLE modem operations.