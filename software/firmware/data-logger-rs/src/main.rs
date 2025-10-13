#![no_std]
#![no_main]

use core::cell::RefCell;
use core::future::{poll_fn, Future};
use core::mem::MaybeUninit;
use core::panic;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Poll;

use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::gpio::{self, Output};
use embassy_rp::i2c::{Async, Error, I2c};
use embassy_rp::pio::{self, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::uart::BufferedUart;
use embassy_rp::usb::{Driver, Instance};
use embassy_rp::{bind_interrupts, config, i2c, peripherals, uart, usb, Peri};
use embassy_rp::peripherals::{I2C0, I2C1};
use embassy_sync::waitqueue::WakerRegistration;
use embassy_time::{Delay, Duration, Ticker, Timer};
use embassy_usb::control::{self, OutResponse, Recipient, RequestType};
use embassy_usb::driver::{Endpoint, EndpointError, EndpointIn};
use embassy_usb::types::InterfaceNumber;
use embassy_usb::{Builder, Handler};
use icm20948_async::{Icm20948, Icm20948Config, IcmBusI2c, Init, MagDisabled};
use mavio::io::{AsyncReceiver, EmbeddedIoAsyncReader, EmbeddedIoAsyncWriter};
use smart_leds::colors::RED;
use smart_leds::RGB8;
use static_cell::StaticCell;

#[cfg(feature = "defmt-serial")]
use {defmt_serial as _, panic_probe as _};

// const DEVICE_INTERFACE_GUIDS:&[&str] = &["{d98ec29a-1655-11f0-bd1e-bc091bcc74fa}"];

// static DEBUG_UART: StaticCell<uart::Uart<'_, Blocking>> = StaticCell::new();

// type SensorPipe = Pipe<NoopRawMutex, 2048>;
// type SensorWriter<'a> = Writer<'a, NoopRawMutex, 2048>;
// static WRITER: StaticCell<SensorWriter> = StaticCell::new();

bind_interrupts!(struct Irqs {
    I2C0_IRQ => i2c::InterruptHandler<peripherals::I2C0>;
    I2C1_IRQ => i2c::InterruptHandler<peripherals::I2C1>;
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
    UART0_IRQ => uart::BufferedInterruptHandler<peripherals::UART0>;
    UART1_IRQ => uart::BufferedInterruptHandler<peripherals::UART1>;
    USBCTRL_IRQ => usb::InterruptHandler<peripherals::USB>;
});

#[embassy_executor::task]
async fn task_camera_trigger(mut pin_trigger: Output<'static>) {
    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        pin_trigger.set_low();
        Timer::after_micros(100).await;
        pin_trigger.set_high();
        ticker.next().await;
    }
}

#[embassy_executor::task]
async fn task_mavlink(uart: BufferedUart) {
    let (uart_tx, uart_rx) = uart.split();
    let uart_rx = EmbeddedIoAsyncReader::new(uart_rx);
    let uart_tx = EmbeddedIoAsyncWriter::new(uart_tx);
    let mut uart_receiver = AsyncReceiver::versioned(uart_rx, mavio::prelude::V2);
    let mut uart_sender = mavio::io::AsyncSender::versioned(uart_tx, mavio::prelude::V2);

    let mavlink_version = mavio::prelude::V2;
    let system_id = 15;
    let component_id = 42;
    let message_id = 24;    // GPS_RAW_INT
    let endpoint = 0;

    let header = mavio::protocol::Header::builder();

    let message = {};

    let frame = mavio::Frame::builder()
        .version(mavlink_version)
        .sequence(0)
        .system_id(system_id)
        .component_id(component_id)
        .message_id(message_id)
        .payload(&[0_u8])
        .crc_extra(0)
        .build();
        // .crc_extra(crc_extra)
        // .endpoint(Endpoint::)
    let result = uart_sender.send(&frame).await;

    struct Heartbeat {
        custom_mode: u32,
        r#type: u8,
        autopilot: u8,
        base_mode: u8,
        system_status: u8,
        mavlink_version: u8,
    }

    // Looks to be LSB first.
    // let timesync_111 = [
    //     0, 0, 0, 0, 0, 0, 0, 0,
    //     217, 136, 28, 2, 194, 4, 0, 0
    // ];
    // let heartbeat_0 = [
    //     0, 0, 0, 0, // custom_mode:
    //     2,          // type: MAV_TYPE: 2: MAV_TYPE_QUADROTOR
    //     3,          // autopilot: MAV_AUTOPILOT: 3: MAV_AUTOPILOT_ARDUPILOTMEGA
    //     81,         // base_mode: MAV_MODE_FLAG: 64: MAV_MODE_FLAG_MANUAL_INPUT_ENABLED + 16: MAV_MODE_FLAG_STABILIZE_ENABLED + 1: MAV_MODE_FLAG_CUSTOM_MODE_ENABLED
    //     3,          // system_status: MAV_STATE: 3: MAV_STATE_STANDBY
    //     3,          // mavlink_version: document mavlink version == 3 in https://github.com/mavlink/mavlink/blob/master/message_definitions/v1.0/minimal.xml
    // ];

    let mut count = 0;
    loop {
        if let Ok(frame) = uart_receiver.recv().await {
            match frame.message_id() {
                // 0 => {},
                // 111 => {},
                _ => info!("message {} {:?}", frame.message_id(), frame.payload().bytes()),
            }
        } else {
            error!("recv");
        }

        count += 1;
    }
}

#[embassy_executor::task]
async fn task_accel_0(i2c: I2c<'static, I2C0, Async>, config: Icm20948Config) {
    task_accel(i2c, config).await
}

#[embassy_executor::task]
async fn task_accel_1(i2c: I2c<'static, I2C1, Async>, config: Icm20948Config) {
    task_accel(i2c, config).await
}

async fn task_accel(i2c: I2c<'static, impl embassy_rp::i2c::Instance, Async>, config: Icm20948Config) {
    let accel0 = Icm20948::new_i2c_from_cfg(i2c, config, Delay);
    accel0.initialize_6dof().await.unwrap_or_else(|_| panic!("imu init failed"));
}

// enum SensorData {
//     Accel(Vector3<i16>),
//     Imu(Data6Dof<i16>),
//     Pt(f32, f32),
//     Sync(bool),
// }

// struct SensorMessage {
//     timestamp: Instant,
//     data: SensorData,
// }

// type SensorChannel = Channel<NoopRawMutex, SensorMessage, 128>;
// type SensorSender<'a> = Sender<'a, NoopRawMutex, SensorMessage, 128>;
type SensorType = Icm20948<IcmBusI2c<I2c<'static, I2C0, Async>>, MagDisabled, Init, Delay, Error>;

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
    const WS2812_LED_COUNT: usize = 1;
    let mut ws2812_state = [smart_leds::RGB8::default(); WS2812_LED_COUNT];

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
    static LOGGER_UART_RX_BUF: StaticCell<[u8; 1024]> = StaticCell::new();
    let logger_uart_tx_buf = &mut LOGGER_UART_TX_BUF.init([0; _])[..];
    let logger_uart_rx_buf = &mut LOGGER_UART_RX_BUF.init([0; _])[..];
    let mut logger_uart_config = uart::Config::default();
    logger_uart_config.baudrate = 921600;

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
    let accel0_i2c_scl = p.PIN_13;
    let accel0_i2c_sda = p.PIN_12;
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
    }
    #[cfg(not(feature = "defmt-serial"))]
    {
        let logger_uart = uart::BufferedUart::new(
            logger_uart,
            logger_uart_pin_tx, logger_uart_pin_rx,
            logger_uart_irq,
            logger_uart_tx_buf,
            logger_uart_rx_buf,
            logger_uart_config,
        );
        let (logger_uart_tx, logger_uart_rx) = logger_uart.split();
        let logger_uart_rx = EmbeddedIoAsyncReader::new(logger_uart_rx);
        let logger_uart_tx = EmbeddedIoAsyncWriter::new(logger_uart_tx);
    }

    let mut ws2812 = {
        let Pio { mut common, sm0, .. } = Pio::new(ws2812_pio, ws2812_irq);
        let program = PioWs2812Program::new(&mut common);
        PioWs2812::new(&mut common, sm0, ws2812_dma, ws2812_pin, &program)
    };

    ws2812_state[0] = RED;
    ws2812.write(&ws2812_state).await;

    let camera_trigger_pin = Output::new(camera_trigger_pin, gpio::Level::High);
    spawner.spawn(task_camera_trigger(camera_trigger_pin)).expect("spawn task_cammera_trigger");

    let mavlink_uart = uart::BufferedUart::new(
        mavlink_uart,
        mavlink_uart_pin_tx, mavlink_uart_pin_rx,
        mavlink_uart_irq,
        mavlink_uart_tx_buf,
        mavlink_uart_rx_buf,
        mavlink_uart_config,
    );
    spawner.spawn(task_mavlink(mavlink_uart)).expect("spawn task_mavlink");

    // let accel0_i2c = i2c::I2c::new_async(accel0_i2c, accel0_i2c_scl, accel0_i2c_sda, accel0_i2c_irq, accel0_i2c_config);
    // spawner.spawn(task_accel_0(accel0_i2c, accel_config)).expect("spawn task_accel_0");

    // let accel1_i2c = i2c::I2c::new_async(accel1_i2c, accel1_i2c_scl, accel1_i2c_sda, accel1_i2c_irq, accel1_i2c_config);
    // spawner.spawn(task_accel_1(accel1_i2c, accel_config)).expect("spawn task_accel_1");

    // let mut pwm_config = pwm::Config::default();
    // pwm_config.top = 10000;
    // pwm_config.divider = 133.into();
    // let mut pwm = Pwm::new_output_a(p.PWM_SLICE2, p.PIN_4, pwm_config);
    // pwm.set_duty_cycle(100).unwrap();

    // static CHANNEL: StaticCell<SensorChannel> = StaticCell::new();
    // let c = CHANNEL.init(SensorChannel::new());
    // static PIPE: StaticCell<SensorPipe> = StaticCell::new();
    // let c = PIPE.init(SensorPipe::new());
    // let (reader, writer) = c.split();

    // spawner.spawn(imu_task(i2c0, i2c0_int, c.sender())).unwrap();
    // spawner.spawn(pt_task(i2c1, i2c1_int, c.sender())).unwrap();
    // spawner.spawn(sync_task(sync_in, c.sender())).unwrap();

    // let rx = c.receiver();
    // let mut rx = reader;

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

    // builder.msos_descriptor(windows_version::WIN8_1, 0);
    // builder.msos_feature(msos::CompatibleIdFeatureDescriptor::new("WINUSB", ""));
    // builder.msos_feature(msos::RegistryPropertyFeatureDescriptor::new(
    //     "DeviceInterfaceGUIDs",
    //     msos::PropertyData::RegMultiSz(DEVICE_INTERFACE_GUIDS),
    // ));

    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new()); //i2c0).await.unwrap());
    let mut class = VendorClass::new(&mut builder, state);

    let mut usb = builder.build();
    let usb_fut = usb.run();

let mut count = 0;
loop {
    Timer::after_millis(333).await;

    ws2812_state[0] = RGB8 {
        r: ((count >> 0) & 1) * 255,
        g: ((count >> 1) & 1) * 255,
        b: ((count >> 2) & 1) * 255,
    };
    ws2812.write(&ws2812_state).await;

    count += 1;
}

    let pump_fut = async {
        loop {
            info!("pump: wait_connection");
            class.wait_connection().await;

            // info!("pump: pump");
            // match pump(&mut class, &mut accel0).await {
            //     Ok(_) => info!("pump: finished"),
            //     Err(_) => {
            //         error!("pump: error");
            //     },
            // }
    /*
                let message = rx.receive().await;

                let buf_len_old = buf.len();
                match message.data {
                    SensorData::Accel(v) => {
                        buf.push(b'A').unwrap();
                        let ticks = message.timestamp.as_ticks();
                        buf.extend_from_slice(&ticks.to_be_bytes()[5..8]).unwrap();
                        buf.extend(v[0].to_be_bytes());
                        buf.extend(v[1].to_be_bytes());
                        buf.extend(v[2].to_be_bytes());
                    },
                    SensorData::Imu(v) => {
                        buf.push(b'I').unwrap();
                        let ticks = message.timestamp.as_ticks();
                        buf.extend_from_slice(&ticks.to_be_bytes()[5..8]).unwrap();
                        buf.extend(v.acc[0].to_be_bytes());
                        buf.extend(v.acc[1].to_be_bytes());
                        buf.extend(v.acc[2].to_be_bytes());
                        buf.extend(v.gyr[0].to_be_bytes());
                        buf.extend(v.gyr[1].to_be_bytes());
                        buf.extend(v.gyr[2].to_be_bytes());
                    },
                    SensorData::Pt(p, t) => {
                        buf.push(b'P').unwrap();
                        let ticks = message.timestamp.as_ticks();
                        buf.extend_from_slice(&ticks.to_be_bytes()[5..8]).unwrap();
                        buf.extend_from_slice(&p.to_be_bytes()).unwrap();
                        buf.extend_from_slice(&t.to_be_bytes()).unwrap();
                    },
                    SensorData::Sync(v) => {
                        let c = if v { b'1' } else { b'0' };
                        buf.push(c).unwrap();
                        let ticks = message.timestamp.as_ticks();
                        buf.extend_from_slice(&ticks.to_be_bytes()[5..8]).unwrap();
                    }
                }

                if buf.len() >= 64 {
                    write_ep.write(&buf[..buf_len_old]).await.ok();
                    buf.rotate_left(buf_len_old);
                    let buf_len = buf.len();
                    buf.truncate(buf_len - buf_len_old);
                }
*/
            info!("pump: looping");
        }
    };

    join(usb_fut, pump_fut).await;
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

async fn pump<'d, T: Instance + 'd>(class: &mut VendorClass<'d, Driver<'d, T>>, sensor: &mut SensorType) -> Result<(), Disconnected> {
    let mut buf = [0u8; 64];

    loop {
        while class.control.streaming.load(Ordering::Relaxed) == false {
            info!("pump: waiting on changed(streaming -> true)");
            class.control.changed().await;
        }

        info!("pump: reset fifo");
        sensor.fifo_reset().await.map_err(|_| Disconnected {})?;

        info!("pump: streaming");
        while class.control.streaming.load(Ordering::Relaxed) == true {
            let fifo_count = sensor.fifo_count().await.map_err(|_| Disconnected {})? as usize;
            // info!("fifo {}", fifo_count);

            if fifo_count >= buf.len() {
                sensor.fifo_read(&mut buf).await.map_err(|_| Disconnected {})?;
                class.write_packet(&buf).await?;
            }
        }

        info!("pump: stop streaming");
    }
}

struct State<'a> {
    control: MaybeUninit<Control<'a>>,
    shared: ControlShared,
}

impl<'a> State<'a> {
    pub fn new() -> Self {
        Self {
            control: MaybeUninit::uninit(),
            shared: ControlShared::new(),
        }
    }
}

struct Control<'a> {
    if_num: InterfaceNumber,
    shared: &'a ControlShared,
}

struct ControlShared {
    streaming: AtomicBool,
    waker: RefCell<WakerRegistration>,
    changed: AtomicBool,
}


impl ControlShared {
    fn new() -> Self {
        ControlShared {
            streaming: AtomicBool::new(false),
            waker: RefCell::new(WakerRegistration::new()),
            changed: AtomicBool::new(false),
        }
    }

    fn changed(&self) -> impl Future<Output = ()> + '_ {
        poll_fn(|cx| {
            if self.changed.load(Ordering::Relaxed) {
                self.changed.store(false, Ordering::Relaxed);
                Poll::Ready(())
            } else {
                self.waker.borrow_mut().register(cx.waker());
                Poll::Pending
            }
        })
    }
}

impl<'a> Control<'a> {
    fn shared(&mut self) -> &'a ControlShared {
        self.shared
    }
}

impl<'d> Handler for Control<'d> {
    /// Called when the USB device has been enabled or disabled.
    fn enabled(&mut self, enabled: bool) {
        info!("usb: enabled: {}", enabled);
    }

    /// Called after a USB reset after the bus reset sequence is complete.
    fn reset(&mut self) {
        info!("usb: reset");

        let shared = self.shared();
        shared.streaming.store(false, Ordering::Relaxed);
        shared.changed.store(true, Ordering::Relaxed);
        shared.waker.borrow_mut().wake();
    }

    /// Called when the host has set the address of the device to `addr`.
    fn addressed(&mut self, addr: u8) {
        info!("usb: addressed: {}", addr);
    }

    /// Called when the host has enabled or disabled the configuration of the device.
    fn configured(&mut self, configured: bool) {
        info!("usb: configured: {}", configured);
    }

    fn control_in<'a>(&'a mut self, req: control::Request, buf: &'a mut [u8]) -> Option<control::InResponse<'a>> {
        info!("usb: control_in: req={}, buf={:a}", req, buf);
        None
    }

    fn control_out(&mut self, req: control::Request, buf: &[u8]) -> Option<control::OutResponse> {
        info!("usb: control_out: req={}, buf={:a}", req, buf);

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
                let shared = self.shared();
                shared.streaming.store(false, Ordering::Relaxed);
                shared.changed.store(true, Ordering::Relaxed);
                shared.waker.borrow_mut().wake();
                return Some(OutResponse::Accepted);
            },
            1 => {
                // Start streaming.
                let shared = self.shared();
                shared.streaming.store(true, Ordering::Relaxed);
                shared.changed.store(true, Ordering::Relaxed);
                shared.waker.borrow_mut().wake();
                return Some(OutResponse::Accepted);
            },
            _ => {
                error!("usb: value {} != 0 or 1", req.value);
                return Some(OutResponse::Rejected);
            }
        }
    }

    fn suspended(&mut self, suspended: bool) {
        info!("usb: suspended: {}", suspended);
    }
}

struct VendorClass<'d, D: embassy_usb::driver::Driver<'d>> {
    write_ep: D::EndpointIn,
    control: &'d ControlShared,
}

impl<'d, D: embassy_usb::driver::Driver<'d>> VendorClass<'d, D> {
    fn new(builder: &mut Builder<'d, D>, state: &'d mut State<'d>) -> Self {
        let mut function = builder.function(0xff, 0, 0);
        let mut interface = function.interface();
        let if_num = interface.interface_number();
        let mut alt = interface.alt_setting(0xff, 0, 0, None);
        let write_ep = alt.endpoint_bulk_in(None, 64);
        drop(function);

        let control = state.control.write(Control {
            shared: &state.shared,
            if_num,
        });
        builder.handler(control);

        let control_shared = &state.shared;

        VendorClass {
            write_ep,
            control: control_shared,
        }
    }
    pub async fn wait_connection(&mut self) {
        self.write_ep.wait_enabled().await
    }

    pub async fn write_packet(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        self.write_ep.write(data).await
    }
}
/*
#[embassy_executor::task]
// async fn imu_task(i2c: I2c<'static, I2C0, Async>, mut int_n: Input<'static>, sender: SensorSender<'static>) {
async fn imu_task(sensor: &'static mut SensorType, writer: SensorWriter<'static>) {

    let mut buf = [0u8; 8 * 10];


    loop {
        // int_n.wait_for_falling_edge().await;
        // let now = Instant::now();

        if let Ok(fifo_count) = sensor.fifo_count().await {
            let fifo_count = fifo_count as usize;
            info!("fifo {}", fifo_count);

            if fifo_count >= buf.len() {
                // let read_len = min(buf.len(), fifo_count);

                if let Ok(()) = sensor.fifo_read(&mut buf).await {
                    match writer.try_write(&buf) {
                        Ok(n) => if n != buf.len() {
                            error!("write incomplete");
                            break;
                        },
                        Err(_) => {
                            error!("writer write failed");
                            break;
                        },
                    }
                } else {
                    error!("fifo read error");
                }
                // } else {
                //     // FIFO overflow, reset the FIFO, try again.
                //     if let Err(_) = sensor.fifo_reset().await {
                //         info!("fifo reset error");
                //     }
                // }
            }
        }
        // if let Ok(measurement) = sensor.read_acc_unscaled().await {
        //     let _ignore = sender.try_send(SensorMessage {
        //         timestamp: now,
        //         data: SensorData::Accel(measurement),
        //     });
        // }
    }
}
*/
/*
#[embassy_executor::task]
async fn pt_task(i2c: I2c<'static, I2C1, Async>, mut int_n: Input<'static>, sender: SensorSender<'static>) {
    let sensor_config = bmp390::Configuration::default();
    let mut sensor = bmp390::Bmp390::try_new(i2c, bmp390::Address::Up, Delay, &sensor_config).await.unwrap();

    loop {
        int_n.wait_for_falling_edge().await;
        let now = Instant::now();

        if let Ok(measurement) = sensor.measure().await {
            let p = measurement.pressure.get::<pascal>();
            let t = measurement.temperature.get::<degree_celsius>();

            let _ignore = sender.try_send(SensorMessage {
                timestamp: now,
                data: SensorData::Pt(p, t),
            });
        }
    }
}
*/
/*
#[embassy_executor::task]
async fn sync_task(mut sync_in: Input<'static>, sender: SensorSender<'static>) {
    loop {
        sync_in.wait_for_any_edge().await;
        let now = Instant::now();
        let new_state = if sync_in.get_level() == gpio::Level::High { true } else { false };

        let _ignore = sender.try_send(SensorMessage {
            timestamp: now,
            data: SensorData::Sync(new_state),
        });
    }
}
*/
