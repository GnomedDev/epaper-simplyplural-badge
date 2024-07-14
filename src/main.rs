#![no_std]
#![no_main]
#![warn(rust_2018_idioms, clippy::pedantic)]

use core::cell::RefCell;

use alloc::{boxed::Box, format};
use draw::FontSize;
use epd_waveshare::{
    epd2in13_v2::{Display2in13 as EpdBuffer, Epd2in13 as EpdDisplay},
    graphics::DisplayRotation,
    prelude::WaveshareDisplay as _,
};
use esp_backtrace as _;
use esp_hal::{
    clock::ClockControl,
    delay::Delay,
    gpio::{self, Io},
    peripherals::{Peripherals, LPWR},
    prelude::*,
    rtc_cntl::Rtc,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
};

use rusttype::Font;
use smoltcp::iface::SocketStorage;

mod draw;
mod simplyplural;
mod wifi;

extern crate alloc;

/// Technically doesn't shutdown the chip, but sleeps with no wakeup sources.
fn coma(lpwr: LPWR, delay: &mut Delay) -> ! {
    Rtc::new(lpwr, None).sleep_deep(&[], delay)
}

#[entry]
fn main() -> ! {
    esp_alloc::heap_allocator!(135 * 1024);

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);

    let clocks = ClockControl::max(system.clock_control).freeze();
    let mut delay = Delay::new(&clocks);

    esp_println::logger::init_logger(log::LevelFilter::Info);

    // Setup the EPD display, over the SPI bus.
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let cs = gpio::Output::new(io.pins.gpio15, gpio::Level::High);
    let busy = gpio::Input::new(io.pins.gpio25, gpio::Pull::None);
    let rst = gpio::Output::new(io.pins.gpio26, gpio::Level::High);
    let dc = gpio::Output::new(io.pins.gpio27, gpio::Level::Low);

    let spi = RefCell::new(
        Spi::new(peripherals.SPI2, 8.MHz(), SpiMode::Mode0, &clocks).with_pins(
            Some(io.pins.gpio13), // sclk
            Some(io.pins.gpio14), // mosi
            gpio::NO_PIN,
            gpio::NO_PIN,
        ),
    );

    let mut spi_bus = embedded_hal_bus::spi::RefCellDevice::new(&spi, cs, delay).unwrap();

    let mut epd = EpdDisplay::new(&mut spi_bus, busy, dc, rst, &mut delay, None)
        .expect("EPaper should be present");

    let mut display = Box::new(EpdBuffer::default());
    display.set_rotation(DisplayRotation::Rotate90);

    let font = Font::try_from_bytes(include_bytes!("../Comfortaa-Medium-Latin.ttf")).unwrap();

    // Setup the WIFI connection and HTTPS client.
    let mut socket_storage = [SocketStorage::EMPTY; 3];
    let _wifi_stack = match wifi::connect(
        &clocks,
        peripherals.TIMG1,
        peripherals.RNG,
        peripherals.RADIO_CLK,
        peripherals.WIFI,
        &mut socket_storage,
    ) {
        Ok(stack) => stack,
        Err(err) => {
            draw::text_to_display(
                &mut display,
                font,
                FontSize::Small,
                &format!("Failed to connect to WIFI: {err:?}"),
            );

            epd.update_and_display_frame(&mut spi_bus, display.buffer(), &mut delay)
                .expect("EPaper should accept update/display requests");

            coma(peripherals.LPWR, &mut delay);
        }
    };

    main_loop(display, delay, &font, |buf| {
        epd.update_and_display_frame(&mut spi_bus, buf, &mut delay)
            .expect("EPaper should accept update/display requests");
    });

    coma(peripherals.LPWR, &mut delay);
}

#[allow(clippy::never_loop)]
fn main_loop(
    mut display: Box<EpdBuffer>,
    _: Delay,
    font: &Font<'static>,
    mut update_screen: impl FnMut(&[u8]),
) {
    loop {
        let text = simplyplural::fetch_current_front_name();

        draw::clear_display(&mut display);
        draw::text_to_display(&mut display, font.clone(), FontSize::Large, text);

        update_screen(display.buffer());

        break;
        // delay.delay(5.secs());
    }
}
