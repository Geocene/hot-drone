#![no_std]
#![no_main]

use core::fmt::Write as _;
use core::mem::{transmute, MaybeUninit};
use core::panic;

use embassy_executor::Spawner;
use embassy_rp::gpio::{self, Output};
use embassy_rp::i2c::{Async, I2c};
use embassy_rp::pio::{self, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::uart::{BufferedUart, BufferedUartRx, BufferedUartTx};
use embassy_rp::usb::{Driver, Instance};
use embassy_rp::{bind_interrupts, config, i2c, peripherals, uart, usb};
use embassy_rp::peripherals::{I2C0, I2C1, PIO0, USB};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::rwlock::RwLock;
use embassy_sync::watch::Watch;
use embassy_time::{Delay, Duration, Instant, Ticker, Timer};
use embassy_usb::control::{self, OutResponse, Recipient, RequestType};
use embassy_usb::driver::{EndpointError, EndpointIn};
use embassy_usb::{Builder, Handler, UsbDevice};
use embedded_io_async::Write;
use heapless::Vec;
use icm20948_async::{Icm20948, Icm20948Config};
use mavio::error::IoError;
use mavio::io::{AsyncReceiver, AsyncSender, EmbeddedIoAsyncReader, EmbeddedIoAsyncWriter};
use mavio::prelude::V2;
use smart_leds::colors::{BLACK, GREEN, RED, WHITE};
use static_cell::StaticCell;

use defmt::*;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

use panic_probe as _;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => i2c::InterruptHandler<peripherals::I2C0>;
    I2C1_IRQ => i2c::InterruptHandler<peripherals::I2C1>;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
    UART0_IRQ => uart::BufferedInterruptHandler<peripherals::UART0>;
    UART1_IRQ => uart::BufferedInterruptHandler<peripherals::UART1>;
    USBCTRL_IRQ => usb::InterruptHandler<peripherals::USB>;
});

type StreamingStateWatch = Watch<CriticalSectionRawMutex, bool, 5>;
type StreamingStateSender<'a> = embassy_sync::watch::Sender<'a, CriticalSectionRawMutex, bool, 5>;
static STREAMING_STATE_WATCH: StreamingStateWatch = Watch::new();

static CAMERA_FILE_PREFIX: RwLock<CriticalSectionRawMutex, Vec<u8, 64>> = RwLock::new(Vec::new());

type Channel = embassy_sync::channel::Channel<NoopRawMutex, Message, 32>;
type Sender = embassy_sync::channel::Sender<'static, NoopRawMutex, Message, 32>;
type Receiver = embassy_sync::channel::Receiver<'static, NoopRawMutex, Message, 32>;

#[derive(Clone, Debug)]
enum Message {
    Accelerometer(Instant, u8, [u8; 60]),
    CameraTrigger(Instant),
    Mavlink(Instant, Vec<u8, 60>),
}

// #[embassy_executor::task]
async fn task_camera_trigger(mut pin_trigger: Output<'static>, uart_writer: &mut BufferedUart, mut ws2812: PioWs2812<'_, PIO0, 0, 1>, sender: Sender) {
    let mut state_receiver = STREAMING_STATE_WATCH.anon_receiver();

    let mut capture_count = 0_usize;
    let mut ticker = Ticker::every(Duration::from_secs(1));

    loop {
        ticker.next().await;

        if let Some(true) = state_receiver.try_get() {
            let timestamp = Instant::now();
            pin_trigger.set_low();
            Timer::after_micros(100).await;
            pin_trigger.set_high();

            // Provide a flash of white when cameras are triggered.
            ws2812.write(&[WHITE]).await;

            let mut buf: Vec<u8, 64> = Vec::new();
            {
                let prefix = CAMERA_FILE_PREFIX.read().await;
                buf.extend_from_slice(&prefix).unwrap();
            }
            writeln!(&mut buf, "_{:05}", capture_count).unwrap();
            uart_writer.write_all(&buf).await.unwrap();
            capture_count += 1;

            if let Err(_) = sender.try_send(Message::CameraTrigger(timestamp)) {
                error!("task_camera_trigger try_send");
            }

            Timer::after_millis(50).await;
            ws2812.write(&[GREEN]).await;
        } else {
            capture_count = 0;

            ws2812.write(&[RED]).await;
            Timer::after_millis(500).await;
            ws2812.write(&[BLACK]).await;
        }
    }
}

const SET_MESSAGE_INTERVAL_GPS_RAW_INT_PAYLOAD: [u8; 33] = [
    0x00, 0x00, 0xc0, 0x41, // param1, f32 == 24.0 (MAVLINK_MSG_ID_GPS_RAW_INT)
    0x00, 0x24, 0x74, 0x49, // param2, f32 == 1_000_000.0
    0x00, 0x00, 0x00, 0x00, // param3
    0x00, 0x00, 0x00, 0x00, // param4
    0x00, 0x00, 0x00, 0x00, // param5
    0x00, 0x00, 0x00, 0x00, // param6
    0x00, 0x00, 0x00, 0x00, // param7
    0xff, 0x01, // command, u16 == 511 (MAV_CMD_SET_MESSAGE_INTERVAL)
    0x00, // target_system
    0x00, // target_component
    0x00, // confirmation
];

const MAVLINK_MSG_ID_HEARTBEAT:         u32 =   0;
const MAVLINK_MSG_ID_PARAM_VALUE:       u32 =  22;
const MAVLINK_MSG_ID_GPS_RAW_INT:       u32 =  24;
// const MAVLINK_MSG_ID_GPS_GLOBAL_ORIGIN: u32 =  49;
const MAVLINK_MSG_ID_COMMAND_LONG:      u32 =  76;
const MAVLINK_MSG_ID_COMMAND_ACK:       u32 =  77;
const MAVLINK_MSG_ID_TIMESYNC:          u32 = 111;
// const MAVLINK_MSG_ID_HOME_POSITION:     u32 = 242;
const MAVLINK_MSG_ID_STATUSTEXT:        u32 = 253;

async fn send_set_message_interval(payload: &[u8], uart_sender: &mut AsyncSender<IoError, EmbeddedIoAsyncWriter<BufferedUartTx>, V2>) {
    info!("send_set_message_interval");
    let frame = mavio::Frame::builder()
        .version(mavio::prelude::V2)
        .sequence(0)
        .system_id(0xff)
        .component_id(0x00)
        .message_id(MAVLINK_MSG_ID_COMMAND_LONG)
        .payload(payload)
        .crc_extra(152)
        .build();
    if let Err(_) = uart_sender.send(&frame).await {
        error!("task_mavlink uart_sender.send MAVLINK_MSG_ID_COMMAND_LONG");
        return;
    }
}

async fn wait_for_ack(uart_receiver: &mut AsyncReceiver<IoError, EmbeddedIoAsyncReader<BufferedUartRx>, V2>) {
    info!("wait_for_ack");
    loop {
        match uart_receiver.recv().await {
            Ok(frame) => {
                if frame.message_id() == MAVLINK_MSG_ID_COMMAND_ACK {
                    break;
                }
            },
            Err(_) => {
                error!("task_mavlink uart_receiver.recv await ack")
            },
        }
    }
}

#[embassy_executor::task]
async fn task_mavlink(uart: BufferedUart, sender: Sender) {
    let (uart_tx, uart_rx) = uart.split();
    let uart_rx = EmbeddedIoAsyncReader::new(uart_rx);
    let uart_tx = EmbeddedIoAsyncWriter::new(uart_tx);
    let mut uart_receiver = AsyncReceiver::versioned(uart_rx, mavio::prelude::V2);
    let mut uart_sender = mavio::io::AsyncSender::versioned(uart_tx, mavio::prelude::V2);

    send_set_message_interval(&SET_MESSAGE_INTERVAL_GPS_RAW_INT_PAYLOAD, &mut uart_sender).await;
    wait_for_ack(&mut uart_receiver).await;

    let mut state_receiver = STREAMING_STATE_WATCH.anon_receiver();

    info!("task_mavlink loop");
    loop {
        if let Ok(frame) = uart_receiver.recv().await {
            let timestamp = Instant::now();
            match frame.message_id() {
                _ => {
                    if let Some(true) = state_receiver.try_get() {
                        for chunk in frame.payload().bytes().chunks(60) {
                            let chunk_vec = Vec::from_slice(chunk.into()).expect("task_mavlink vec::from_slice");
                            let message = Message::Mavlink(timestamp, chunk_vec);
                            if let Err(_) = sender.try_send(message) {
                                error!("task_mavlink try_send");
                            }
                        }
                    }
                },
            }
            match frame.message_id() {
                MAVLINK_MSG_ID_HEARTBEAT
                | MAVLINK_MSG_ID_PARAM_VALUE
                | MAVLINK_MSG_ID_GPS_RAW_INT
                // | MAVLINK_MSG_ID_COMMAND_ACK
                | MAVLINK_MSG_ID_TIMESYNC
                | MAVLINK_MSG_ID_STATUSTEXT => {},
                _ => {
                    info!("task_mavlink frame message_id {} payload_len={} {:#02x}", frame.message_id(), frame.payload_length(), frame.payload().bytes());
                },
            }
        } else {
            error!("recv");
        }
    }
}

#[embassy_executor::task]
async fn task_accel_0(i2c: I2c<'static, I2C0, Async>, config: Icm20948Config, sender: Sender) {
    task_accel(i2c, config, 0, sender).await
}

#[embassy_executor::task]
async fn task_accel_1(i2c: I2c<'static, I2C1, Async>, config: Icm20948Config, sender: Sender) {
    task_accel(i2c, config, 1, sender).await
}

async fn task_accel(i2c: I2c<'static, impl embassy_rp::i2c::Instance, Async>, config: Icm20948Config, ordinal: u8, sender: Sender) {
    let sensor = Icm20948::new_i2c_from_cfg(i2c, config, Delay);
    let mut sensor = match sensor.initialize_6dof().await {
        Ok(a) => a,
        Err(e) => {
            match e.0 {
                icm20948_async::IcmError::BusError(e) => error!("task_accel{} initialize_6dof bus error {}", ordinal, e),
                icm20948_async::IcmError::ImuSetupError => error!("task_accel{} initialize_6dof IMU setup error", ordinal),
                icm20948_async::IcmError::MagSetupError => error!("task_accel{} initialize_6dof mag setup error", ordinal),
            }
            return;
        },
    };

    let mut receiver = STREAMING_STATE_WATCH.receiver().expect("task_accel_x streaming_state_watch receiver");

    loop {
        if let Err(e) = sensor.fifo_hold_in_reset().await {
            error!("task_accel{} fifo_hold_in_reset {}", ordinal, e);
            return;
        }

        loop {
            let streaming = receiver.changed().await;
            if streaming {
                break;
            }
        }

        if let Err(e) = sensor.fifo_reset().await {
            error!("task_accel{} fifo_reset {}", ordinal, e);
            return;
        }

        let mut buf = [0_u8; 6 * 10];

        while receiver.try_changed().is_none() {
            let fifo_count = match sensor.fifo_count().await {
                Ok(v) => v as usize,
                Err(_) => {
                    error!("task_accel{} fifo_count", ordinal);
                    return;
                },
            };

            if fifo_count >= buf.len() {
                match sensor.fifo_read(&mut buf).await {
                    Err(_) => {
                        error!("task_accel{} fifo_read", ordinal);
                        return;
                    },
                    _ => {
                        let timestamp = Instant::now();
                        if let Err(_) = sender.try_send(Message::Accelerometer(timestamp, ordinal, buf)) {
                            error!("task_accel{} try_send", ordinal);
                        }
                    },
                }
            }
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let config = config::Config::default();
    let p = embassy_rp::init(config);

    ///////////////////////////////////////////////////////////////////
    // Low-Level Peripheral Mappings

    let ws2812_pio = p.PIO0;
    let ws2812_irq = Irqs;
    let ws2812_dma = p.DMA_CH4;
    let ws2812_pin = p.PIN_17;

    // Pin used to enable the 1.8 V regulator to the level shifter for the
    // camera trigger signal.
    let supply_1v8_enable_pin = p.PIN_3;

    // Pin used to trigger the Arducam B0262 IMX477 camera modules.
    // The B0262 circuit board has "X" (external trigger) and "G" (ground)
    // pads that can be used, along with the appropriate `imx477` kernel module
    // settings, to trigger the sensor to capture an image.
    let camera_trigger_pin = p.PIN_4;

    // Interface to the camera computers, used to inform the camera computers
    // of while file name, timestamp, sequence number, or whatever should be associated
    // by the most recently triggered image capture.
    let logger_uart = p.UART0;
    let logger_uart_pin_tx = p.PIN_0;
    let logger_uart_pin_rx = p.PIN_1;
    let logger_uart_irq = Irqs;
    static LOGGER_UART_TX_BUF: StaticCell<[u8;  256]> = StaticCell::new();
    static LOGGER_UART_RX_BUF: StaticCell<[u8;   16]> = StaticCell::new();
    let logger_uart_tx_buf = &mut LOGGER_UART_TX_BUF.init([0; _])[..];
    let logger_uart_rx_buf = &mut LOGGER_UART_RX_BUF.init([0; _])[..];
    let mut logger_uart_config = uart::Config::default();
    logger_uart_config.baudrate = 115200;

    let mavlink_uart = p.UART1;
    let mavlink_uart_pin_tx = p.PIN_8;
    let mavlink_uart_pin_rx = p.PIN_9;
    let mavlink_uart_irq = Irqs;
    static MAVLINK_UART_TX_BUF: StaticCell<[u8;  256]> = StaticCell::new();
    static MAVLINK_UART_RX_BUF: StaticCell<[u8; 1024]> = StaticCell::new();
    let mavlink_uart_tx_buf = &mut MAVLINK_UART_TX_BUF.init([0; _])[..];
    let mavlink_uart_rx_buf = &mut MAVLINK_UART_RX_BUF.init([0; _])[..];
    let mut mavlink_uart_config = uart::Config::default();
    mavlink_uart_config.baudrate = 921600;

    let accel_config = Icm20948Config {
        gyr_unit: icm20948_async::GyrUnit::Rps,
        gyr_dlp: icm20948_async::GyrDlp::Disabled,
        gyr_odr: 0,
        acc_unit: icm20948_async::AccUnit::Gs,
        acc_dlp: icm20948_async::AccDlp::Disabled,
        acc_range: icm20948_async::AccRange::Gs16,
        acc_odr: 0,
        ..Default::default()
    };

    let accel0_i2c = p.I2C0;
    let accel0_i2c_scl = p.PIN_29;
    let accel0_i2c_sda = p.PIN_28;
    let accel0_i2c_irq = Irqs;
    let mut accel0_i2c_config = i2c::Config::default();
    accel0_i2c_config.frequency = 400_000;

    let accel1_i2c = p.I2C1;
    let accel1_i2c_scl = p.PIN_7;
    let accel1_i2c_sda = p.PIN_6;
    let accel1_i2c_irq = Irqs;
    let mut accel1_i2c_config = i2c::Config::default();
    accel1_i2c_config.frequency = 400_000;

    ///////////////////////////////////////////////////////////////////
    // Peripheral Configuration

    #[cfg(feature = "defmt-serial")]
    {
        static SERIAL: StaticCell<uart::Uart<'_, embassy_rp::uart::Blocking>> = StaticCell::new();
        let serial = uart::Uart::new_blocking(logger_uart, logger_uart_pin_tx, logger_uart_pin_rx, logger_uart_config);
        defmt_serial::defmt_serial(SERIAL.init(serial));
        info!("defmt ready");
    }
    #[cfg(not(feature = "defmt-serial"))]
    let logger_uart = {
        static SERIAL: StaticCell<uart::BufferedUart> = StaticCell::new();
        let logger_uart = uart::BufferedUart::new(
            logger_uart,
            logger_uart_pin_tx, logger_uart_pin_rx,
            logger_uart_irq,
            logger_uart_tx_buf,
            logger_uart_rx_buf,
            logger_uart_config,
        );
        SERIAL.init(logger_uart)
    };

    let mut ws2812 = {
        let Pio { mut common, sm0, .. } = Pio::new(ws2812_pio, ws2812_irq);
        let program = PioWs2812Program::new(&mut common);
        PioWs2812::new(&mut common, sm0, ws2812_dma, ws2812_pin, &program)
    };
    ws2812.write(&[RED]).await;

    let channel = {
        static CHANNEL: StaticCell<Channel> = StaticCell::new();
        CHANNEL.init(Channel::new())
    };

    // Set state of camera trigger pin before enabling regulator that powers the level translator.
    let camera_trigger_pin = Output::new(camera_trigger_pin, gpio::Level::High);
    let _supply_1v8_enable_pin = Output::new(supply_1v8_enable_pin, gpio::Level::High);

    let mavlink_uart = uart::BufferedUart::new(
        mavlink_uart,
        mavlink_uart_pin_tx, mavlink_uart_pin_rx,
        mavlink_uart_irq,
        mavlink_uart_tx_buf,
        mavlink_uart_rx_buf,
        mavlink_uart_config,
    );

    let accel0_i2c = i2c::I2c::new_async(accel0_i2c, accel0_i2c_scl, accel0_i2c_sda, accel0_i2c_irq, accel0_i2c_config);
    let accel1_i2c = i2c::I2c::new_async(accel1_i2c, accel1_i2c_scl, accel1_i2c_sda, accel1_i2c_irq, accel1_i2c_config);

    // let mut pwm_config = pwm::Config::default();
    // pwm_config.top = 10000;
    // pwm_config.divider = 133.into();
    // let mut pwm = Pwm::new_output_a(p.PWM_SLICE2, p.PIN_4, pwm_config);
    // pwm.set_duty_cycle(100).unwrap();

    let driver = usb::Driver::new(p.USB, Irqs);

    let config = {
        let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("Embassy");
        config.product = Some("Sensor streamer");
        config.serial_number = Some("12234269");
        config.max_power = 100;
        config.max_packet_size_0 = 64;
        config
    };

    let mut builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static MSOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

        let builder = embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            MSOS_DESCRIPTOR.init([0; 256]),
            CONTROL_BUF.init([0; 64]),
        );
        builder
    };

    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());
    static CLASS: StaticCell<VendorClass<Driver<USB>>> = StaticCell::new();
    let class = CLASS.init(VendorClass::new(&mut builder, state));

    let usb = builder.build();
    spawner.spawn(usb_run(usb)).expect("spawn usb_run");
    spawner.spawn(usb_data_pump(class, channel.receiver())).expect("span usb_data_pump");

    // spawner.spawn(task_camera_trigger(camera_trigger_pin, channel.sender())).expect("spawn task_cammera_trigger");
    spawner.spawn(task_mavlink(mavlink_uart, channel.sender())).expect("spawn task_mavlink");
    spawner.spawn(task_accel_0(accel0_i2c, accel_config, channel.sender())).expect("spawn task_accel_0");
    spawner.spawn(task_accel_1(accel1_i2c, accel_config, channel.sender())).expect("spawn task_accel_1");

    task_camera_trigger(camera_trigger_pin, logger_uart, ws2812, channel.sender()).await;
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(value: EndpointError) -> Self {
        match value {
            EndpointError::BufferOverflow => panic!("buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

#[embassy_executor::task]
async fn usb_run(mut device: UsbDevice<'static, Driver<'static, USB>>) {
    device.run().await;
}

#[embassy_executor::task]
async fn usb_data_pump(class: &'static mut VendorClass<'static, Driver<'static, USB>>, receiver: Receiver) {
    if let Err(_) = pump(class, receiver).await {
        error!("usb_data_pump pump disconnected");
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct UsbTimestamp([u8; 2]);

impl From<Instant> for UsbTimestamp {
    fn from(value: Instant) -> Self {
        let ticks = value.as_ticks() / 1000;
        Self([
            (ticks >>  8) as u8,
            (ticks >>  0) as u8,
        ])
    }
}

#[derive(Copy, Clone, Debug)]
enum UsbMessageId {
    Accelerometer(u8),
    CameraTrigger,
    Mavlink,
}

impl From<UsbMessageId> for u8 {
    fn from(value: UsbMessageId) -> Self {
        match value {
            UsbMessageId::Accelerometer(n) => 0xc0 | n,
            UsbMessageId::CameraTrigger => 0xce,
            UsbMessageId::Mavlink => 0xcf,
        }
    }
}

#[repr(C, packed)]
struct UsbAccelDataFrame {
    message: u8,
    payload_length: u8,
    timestamp: UsbTimestamp,
    data: [u8; 60],
}

#[repr(C, packed)]
struct UsbCameraTriggerFrame {
    message: u8,
    payload_length: u8,
    timestamp: UsbTimestamp,
}

#[repr(C, packed)]
struct UsbMavlinkFrameHeader {
    message: u8,
    payload_length: u8,
    timestamp: UsbTimestamp,
}

async fn pump<'d, T: Instance + 'd>(class: &mut VendorClass<'d, Driver<'d, T>>, receiver: Receiver) -> Result<(), Disconnected> {
    let mut is_streaming = STREAMING_STATE_WATCH.anon_receiver();

    loop {
        let message = receiver.receive().await;

        if let Some(true) = is_streaming.try_get() {
            match message {
                Message::Accelerometer(instant, ordinal, data) => {
                    let frame = UsbAccelDataFrame {
                        message: UsbMessageId::Accelerometer(ordinal).into(),
                        payload_length: data.len() as u8,
                        timestamp: instant.into(),
                        data,
                    };
                    let frame_buf: [u8; 64] = unsafe { transmute(frame) };
                    class.write_packet(&frame_buf).await?;
                },
                Message::CameraTrigger(instant) => {
                    let frame = UsbCameraTriggerFrame {
                        message: UsbMessageId::CameraTrigger.into(),
                        payload_length: 0,
                        timestamp: instant.into(),
                    };
                    let frame_buf: [u8; 4] = unsafe { transmute(frame) };
                    class.write_packet(&frame_buf).await?;
                },
                Message::Mavlink(instant, data) => {
                    let frame = UsbMavlinkFrameHeader {
                        message: UsbMessageId::Mavlink.into(),
                        payload_length: data.len() as u8,
                        timestamp: instant.into(),
                    };
                    let frame_buf: [u8; 4] = unsafe { transmute(frame) };
                    let mut frame_buf: Vec<u8, 64> = Vec::from_slice(&frame_buf).expect("pump vec::from_slice");
                    frame_buf.extend(data);
                    class.write_packet(frame_buf.as_slice()).await?;
                },
            }
        }
    }
}

struct State<'a> {
    control: MaybeUninit<Control<'a>>,
}

impl State<'_> {
    pub fn new() -> Self {
        Self {
            control: MaybeUninit::uninit(),
        }
    }
}

struct Control<'a> {
    state_sender: StreamingStateSender<'a>,
}

impl Handler for Control<'_> {
    /// Called when the USB device has been enabled or disabled.
    fn enabled(&mut self, enabled: bool) {
        if !enabled {
            self.state_sender.send(false);
        }
    }

    /// Called after a USB reset after the bus reset sequence is complete.
    fn reset(&mut self) {
        self.state_sender.send(false);
    }

    /// Called when the host has set the address of the device to `addr`.
    // fn addressed(&mut self, addr: u8) {
    //     info!("usb: addressed: {}", addr);
    // }

    /// Called when the host has enabled or disabled the configuration of the device.
    fn configured(&mut self, configured: bool) {
        if !configured {
            self.state_sender.send(false);
        }
    }

    // fn control_in<'a>(&'a mut self, req: control::Request, buf: &'a mut [u8]) -> Option<control::InResponse<'a>> {
    //     // info!("usb: control_in: req={}, buf={:a}", req, buf);
    //     None
    // }

    fn control_out(&mut self, req: control::Request, buf: &[u8]) -> Option<control::OutResponse> {
        // info!("usb: control_out: req={}, buf={:a}", req, buf);

        if req.request_type != RequestType::Vendor {
            error!("usb: request_type {} != Vendor", req.request_type);
            return None;
        }
        if req.recipient != Recipient::Interface {
            error!("usb: recipient {} != Interface", req.recipient);
            return None;
        }
        if req.index != 0 {
            error!("usb: index {} != 0", req.index);
            return None;
        }
        if req.request != 0 {
            error!("usb: request {} != 0", req.request);
            return Some(OutResponse::Rejected);
        }

        match req.value {
            0 => {
                // Stop streaming.
                self.state_sender.send(false);
                return Some(OutResponse::Accepted);
            },
            1 => {
                // Start streaming.
                loop {
                    if let Ok(mut prefix) = CAMERA_FILE_PREFIX.try_write() {
                        prefix.clear();
                        prefix.extend_from_slice(buf).unwrap();
                        break;
                    }
                }
                self.state_sender.send(true);
                return Some(OutResponse::Accepted);
            },
            _ => {
                error!("usb: value {} != 0 or 1", req.value);
                return Some(OutResponse::Rejected);
            }
        }
    }

    fn suspended(&mut self, suspended: bool) {
        if suspended {
            self.state_sender.send(false);
        }
    }
}

struct VendorClass<'d, D: embassy_usb::driver::Driver<'d>> {
    write_ep: D::EndpointIn,
}

impl<'d, D: embassy_usb::driver::Driver<'d>> VendorClass<'d, D> {
    fn new(builder: &mut Builder<'d, D>, state: &'d mut State<'d>) -> Self {
        let mut function = builder.function(0xff, 0, 0);
        let mut interface = function.interface();
        let _if_num = interface.interface_number();
        let mut alt = interface.alt_setting(0xff, 0, 0, None);
        let write_ep = alt.endpoint_bulk_in(None, 64);
        drop(function);

        let control = state.control.write(Control {
            state_sender: STREAMING_STATE_WATCH.sender(),
        });
        builder.handler(control);

        VendorClass {
            write_ep,
        }
    }

    // pub async fn wait_connection(&mut self) {
    //     self.write_ep.wait_enabled().await
    // }

    pub async fn write_packet(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        self.write_ep.write(data).await
    }
}
