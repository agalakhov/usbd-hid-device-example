
#![no_std]
#![no_main]

extern crate panic_semihosting;
use cortex_m::asm::delay;
use cortex_m_rt::entry;
use lsm303dlhc::Lsm303dlhc;
use stm32f3xx_hal::{prelude::*, stm32, i2c::I2c, usb::{Peripheral as UsbPeripheral, UsbBus}};

mod led;
mod report;
use led::*;

const SENSITIVITY: i16 = 128;

#[entry]
fn main() -> ! {
    let dp = stm32::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(48.mhz())
        .pclk1(24.mhz())
        .freeze(&mut flash.acr);

    assert!(clocks.usbclk_valid());

    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb);
    let mut gpiob = dp.GPIOB.split(&mut rcc.ahb);
    let gpioe = dp.GPIOE.split(&mut rcc.ahb);

    let leds = Leds::new(gpioe);

    // F3 Discovery board has a pull-up resistor on the D+ line.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
    usb_dp.set_low().ok();
    delay(clocks.sysclk().0 / 100);

    let usb_dm = gpioa.pa11.into_af14(&mut gpioa.moder, &mut gpioa.afrh);
    let usb_dp = usb_dp.into_af14(&mut gpioa.moder, &mut gpioa.afrh);
    let usb = UsbPeripheral {
        usb: dp.USB,
        pin_dm: usb_dm,
        pin_dp: usb_dp,
    };
    let usb_bulder = UsbBus::new(usb);

    usb::init_globals(usb_bulder, leds);

    let scl = gpiob.pb6.into_af4(&mut gpiob.moder, &mut gpiob.afrl);
    let sda = gpiob.pb7.into_af4(&mut gpiob.moder, &mut gpiob.afrl);
    let i2c = I2c::i2c1(dp.I2C1, (scl, sda), 400.khz(), clocks, &mut rcc.apb1);
    let mut sensor = Lsm303dlhc::new(i2c).unwrap();

    let button = gpioa.pa0.into_pull_down_input(&mut gpioa.moder, &mut gpioa.pupdr);

    loop {
        let pressed = button.is_high().unwrap();
        let accel = sensor.accel().unwrap();
        let (x, y) = (accel.x / SENSITIVITY, accel.y / SENSITIVITY);
        usb::send(pressed, x, y);
    }
}


mod usb {
    use core::cell::RefCell;
    use stm32f3xx_hal::{stm32::{interrupt, Interrupt}, usb::Peripheral};
    use cortex_m::{peripheral::NVIC, interrupt::{Mutex, free}};
    use usb_device::prelude::*;
    use usb_device::bus::UsbBusAllocator;
    use super::UsbBus;
    use usbd_hid_device::{USB_CLASS_HID, Hid};

    use super::led::*;
    use super::report::*;

    pub fn init_globals(usb_alloc: UsbBusAllocator<UsbBus<Peripheral>>, leds: Leds) {
        static mut USB_ALLOC: Option<UsbBusAllocator<UsbBus<Peripheral>>> = None;
        free(move |cs| {
            let usb_alloc = unsafe {
                USB_ALLOC = Some(usb_alloc);
                USB_ALLOC.as_ref().unwrap()
            };

            let hid = Hid::new(&usb_alloc, 10);
            //let mut hid = Hid::new(&usb_bus);
            let device = UsbDeviceBuilder::new(&usb_alloc, UsbVidPid(0x16c0, 0x27da))
                .product("HID example mouse")
                .device_class(USB_CLASS_HID)
                .build();

            let usb = Usb {
                hid,
                device,
                leds,
            };

            USB.borrow(&cs).replace(Some(usb));
        });
        unsafe {
            NVIC::unmask(Interrupt::USB_LP_CAN_RX0);
        }
    }

    pub fn send(pressed: bool, x: i16, y: i16) {
        free(move |cs| {
            static mut PRESSED: bool = false;

            let mut borrow = USB.borrow(&cs).borrow_mut();
            let usb = &mut borrow.as_mut().unwrap();

            // ignore too small movements
            let (x, y) = if x.abs() + y.abs() < 10 { (0, 0) } else { (x, y) };

            usb.leds[Direction::North].set(x > 0 && y == 0);
            usb.leds[Direction::South].set(x < 0 && y == 0);
            usb.leds[Direction::West].set(y > 0 && x == 0);
            usb.leds[Direction::East].set(y < 0 && x == 0);
            usb.leds[Direction::Northeast].set(x > 0 && y < 0);
            usb.leds[Direction::Southeast].set(x < 0 && y < 0);
            usb.leds[Direction::Northwest].set(x > 0 && y > 0);
            usb.leds[Direction::Southwest].set(x < 0 && y > 0);

            if pressed != unsafe { PRESSED } || x != 0 || y != 0 {
                let report = MouseReport::new(pressed, -y as i8, -x as i8);
                usb.hid.send_report(&report).ok();
            }

            unsafe { PRESSED = pressed; }
        })
    }

    #[interrupt]
    fn USB_LP_CAN_RX0() {
        usb_interrupt();
    }

    struct Usb {
        device: UsbDevice<'static, UsbBus<Peripheral>>,
        hid: Hid<'static, MouseReport, UsbBus<Peripheral>>,
        leds: Leds,
    }

    static USB: Mutex<RefCell<Option<Usb>>> = Mutex::new(RefCell::new(None));

    fn usb_interrupt() {
        free(move |cs| {
            let mut borrow = USB.borrow(&cs).borrow_mut();
            let usb = &mut borrow.as_mut().unwrap();
            usb.device.poll(&mut [&mut usb.hid]);
        })
    }

}
