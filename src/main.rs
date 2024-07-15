#![no_std]
#![no_main]
#![warn(rust_2018_idioms, clippy::pedantic)]

use alloc::{boxed::Box, format};
use core::{cell::RefCell, str::FromStr as _};

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
use esp_wifi::{wifi::WifiStaDevice, wifi_interface::Socket};

use embedded_io::Write as _;
use epd_waveshare::{
    epd2in13_v2::{Display2in13 as EpdBuffer, Epd2in13 as EpdDisplay},
    graphics::DisplayRotation,
    prelude::WaveshareDisplay as _,
};

use rusttype::Font;
use smoltcp::iface::SocketStorage;

mod draw;
mod simplyplural;
mod wifi;

extern crate alloc;

type SocketError = <Socket<'static, 'static, WifiStaDevice> as embedded_io::ErrorType>::Error;

/// Technically doesn't shutdown the chip, but sleeps with no wakeup sources.
fn coma(lpwr: LPWR, delay: &mut Delay) -> ! {
    Rtc::new(lpwr, None).sleep_deep(&[], delay)
}

#[entry]
fn main() -> ! {
    esp_alloc::heap_allocator!(100 * 1000);

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);

    let clocks = ClockControl::max(system.clock_control).freeze();
    let mut delay = Delay::new(&clocks);

    esp_println::logger::init_logger(log::LevelFilter::Info);

    let proxy_ip = wifi::PROXY_IP.parse().unwrap();
    let proxy_port = wifi::PROXY_PORT.parse().unwrap();

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
    let mut display_error = |text: &str| {
        log::info!("{text}");
        draw::text_to_display(&mut display, font.clone(), text);

        epd.update_and_display_frame(&mut spi_bus, display.buffer(), &mut delay)
            .expect("EPaper should accept update/display requests");
    };

    // Setup the WIFI connection and socket client.
    let mut socket_storage = [SocketStorage::EMPTY; 3];
    let wifi_stack = match wifi::connect(
        &clocks,
        peripherals.TIMG1,
        peripherals.RNG,
        peripherals.RADIO_CLK,
        peripherals.WIFI,
        &mut socket_storage,
    ) {
        Ok(stack) => stack,
        Err(err) => {
            display_error(&format!("Failed to connect to wifi: {err:?}"));
            coma(peripherals.LPWR, &mut delay);
        }
    };

    let mut recv_buf = [0; 1024];
    let mut send_buf = [0; 1024];
    let mut socket = wifi_stack.get_socket(&mut recv_buf, &mut send_buf);

    log::info!("Opening socket to SP proxy");
    let open_res = socket.open(proxy_ip, proxy_port);
    if let Err(err) = open_res.and_then(|()| socket.write_all(wifi::PROXY_KEY.as_bytes())) {
        display_error(&format!("Failed to connect to proxy: {err:?}"));
        coma(peripherals.LPWR, &mut delay);
    }

    log::info!("Starting main loop");
    main_loop(&mut display, delay, &font, &mut socket, |buf| {
        epd.update_and_display_frame(&mut spi_bus, buf, &mut delay)
            .expect("EPaper should accept update/display requests");
    });

    coma(peripherals.LPWR, &mut delay);
}

#[allow(clippy::never_loop)]
fn main_loop(
    display: &mut EpdBuffer,
    delay: Delay,
    font: &Font<'static>,
    socket: &mut Socket<'_, '_, WifiStaDevice>,
    mut update_screen: impl FnMut(&[u8]),
) {
    let mut prev_text = heapless::String::new();
    loop {
        let text = match simplyplural::fetch_current_front_name(socket) {
            Ok(text) => text,
            Err(err) => {
                let mut string = format!("Err: {err:?}");
                string.truncate(32);

                heapless::String::from_str(&string).unwrap()
            }
        };

        if text != prev_text {
            draw::clear_display(display);
            draw::text_to_display(display, font.clone(), text.trim_end());

            update_screen(display.buffer());
            prev_text = text;
        }

        delay.delay(10.secs());
    }
}
