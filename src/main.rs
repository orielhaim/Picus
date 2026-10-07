#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_usb::{Builder, Config, UsbVersion};
use panic_halt as _;
use static_cell::StaticCell;

use picus::mtp::responder::Responder;
use picus::usb::{Control, MtpClass, ResetSignal};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    static RESPONDER: StaticCell<Responder> = StaticCell::new();
    static CONTROL: StaticCell<Control> = StaticCell::new();
    static RESET: StaticCell<ResetSignal> = StaticCell::new();
    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let responder = RESPONDER.init(Responder::new());
    let reset: &'static ResetSignal = RESET.init(ResetSignal::new());
    let control = CONTROL.init(Control::new(0, reset));

    let driver = Driver::new(p.USB, Irqs);

    let mut config = Config::new(0x2E8A, 0x4020);
    config.manufacturer = Some("p2r3");
    config.product = Some("USB of Babel");
    config.serial_number = Some("BABELUSB0001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    config.bcd_usb = UsbVersion::Two;
    config.composite_with_iads = false;
    config.device_class = 0x00;
    config.device_sub_class = 0x00;
    config.device_protocol = 0x00;

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        MSOS_DESC.init([0; 256]),
        CONTROL_BUF.init([0; 64]),
    );

    let mut class = MtpClass::new(&mut builder, responder, control, reset);
    let mut usb = builder.build();

    embassy_futures::join::join(usb.run(), class.run()).await;
}
