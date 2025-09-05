#![no_std]
#![no_main]

use core::cell::RefCell;
use core::future::{poll_fn, Future};
use core::mem::MaybeUninit;
use core::panic;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Poll;

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::i2c::{Async, Error, I2c};
use embassy_rp::usb::{Driver, Instance};
use embassy_rp::{bind_interrupts, config, i2c, peripherals, uart, usb};
use embassy_rp::peripherals::{I2C0, UART0};
use embassy_sync::waitqueue::WakerRegistration;
use embassy_time::Delay;
use embassy_usb::control::{self, OutResponse, Recipient, RequestType};
use embassy_usb::driver::{Endpoint, EndpointError, EndpointIn};
use embassy_usb::types::InterfaceNumber;
use embassy_usb::{Builder, Handler};
use icm20948_async::{Icm20948, IcmBusI2c, Init, MagDisabled};
use static_cell::StaticCell;

// use {defmt_rtt as _, panic_probe as _};
use {defmt_serial as _, panic_probe as _};

// const DEVICE_INTERFACE_GUIDS:&[&str] = &["{d98ec29a-1655-11f0-bd1e-bc091bcc74fa}"];

static DEBUG_UART: StaticCell<uart::Uart<UART0, uart::Blocking>> = StaticCell::new();

// type SensorPipe = Pipe<NoopRawMutex, 2048>;
// type SensorWriter<'a> = Writer<'a, NoopRawMutex, 2048>;
// static WRITER: StaticCell<SensorWriter> = StaticCell::new();

bind_interrupts!(struct Irqs {
    I2C0_IRQ => i2c::InterruptHandler<peripherals::I2C0>;
    I2C1_IRQ => i2c::InterruptHandler<peripherals::I2C1>;
    UART0_IRQ => uart::InterruptHandler<peripherals::UART0>;
    USBCTRL_IRQ => usb::InterruptHandler<peripherals::USB>;
});

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
async fn main(_spawner: Spawner) {
    let config = config::Config::default();
    let p = embassy_rp::init(config);

    let mut uart_config = uart::Config::default();
    uart_config.baudrate = 921600;
    let uart = uart::Uart::new_blocking(p.UART0, p.PIN_0, p.PIN_1, /*Irqs, p.DMA_CH0, p.DMA_CH1,*/ uart_config);
    defmt_serial::defmt_serial(DEBUG_UART.init(uart));

    let mut i2c_config = i2c::Config::default();
    i2c_config.frequency = 400_000;
    let i2c = i2c::I2c::new_async(p.I2C0, p.PIN_13, p.PIN_12, Irqs, i2c_config);
    // let i2c0_int = Input::new(p.PIN_6, Pull::None);
    // let Ok(mut sensor) = sensor else { panic!("imu new_i2c") };
    let sensor = Icm20948::new_i2c(i2c, Delay)
        .gyr_unit(icm20948_async::GyrUnit::Rps)
        .gyr_dlp(icm20948_async::GyrDlp::Disabled)
        .gyr_odr(0)
        .acc_unit(icm20948_async::AccUnit::Gs)
        .acc_dlp(icm20948_async::AccDlp::Disabled)
        .acc_range(icm20948_async::AccRange::Gs16)
        .acc_odr(0);
    let mut sensor = sensor.initialize_6dof().await.unwrap_or_else(|_| panic!("imu init failed"));

/*
    let mut i2c1_config = i2c::Config::default();
    i2c1_config.frequency = 400_000;
    let i2c1 = i2c::I2c::new_async(p.I2C1, p.PIN_3, p.PIN_2, Irqs, i2c1_config);
    let i2c1_int = Input::new(p.PIN_7, Pull::None);
*/
    // let sync_in = Input::new(p.PIN_29, Pull::Up);

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

    let pump_fut = async {
        loop {
            info!("pump: wait_connection");
            class.wait_connection().await;

            info!("pump: pump");
            match pump(&mut class, &mut sensor).await {
                Ok(_) => info!("pump: finished"),
                Err(_) => {
                    error!("pump: error");
                },
            }
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
        let write_ep = alt.endpoint_bulk_in(64);
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
