#![no_std]
#![no_main]
#![warn(rust_2018_idioms, clippy::pedantic)]
#![feature(type_alias_impl_trait, impl_trait_in_assoc_type, concat_bytes)]

use alloc::format;
use core::{cell::RefCell, str::FromStr as _};
use embassy_net::{
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_time::Timer;
use embedded_hal_bus::spi::RefCellDevice;
use simplyplural::HttpClient;

use embassy_executor::Spawner;
use epd_waveshare::{
    epd2in13_v2::Display2in13 as EpdBuffer, graphics::DisplayRotation,
    prelude::WaveshareDisplay as _,
};
use esp_backtrace as _;
use esp_hal::{
    clock::{ClockControl, CpuClock},
    delay::Delay,
    gpio::{self},
    peripherals::{Peripherals, LPWR, SPI2},
    prelude::*,
    rtc_cntl::Rtc,
    spi::{FullDuplexMode, SpiMode},
    system::SystemControl,
    timer::timg::TimerGroup,
};
use rusttype::Font;

mod draw;
mod simplyplural;
mod wifi;

extern crate alloc;

#[macro_export]
macro_rules! make_static {
    ($t:ty, $val:expr) => ($crate::make_static!($t, $val,));
    ($t:ty, $val:expr, $(#[$m:meta])*) => {{
        $(#[$m])*
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        STATIC_CELL.init_with(|| $val)
    }};
}

/// Technically doesn't shutdown the chip, but sleeps with no wakeup sources.
fn coma(lpwr: LPWR) -> ! {
    Rtc::new(lpwr).sleep_deep(&[])
}

type Spi = esp_hal::spi::master::Spi<'static, SPI2, FullDuplexMode>;
type SpiBus = RefCellDevice<'static, Spi, gpio::Output<'static, gpio::Gpio15>, Delay>;
type EpdDisplay = epd_waveshare::epd2in13_v2::Epd2in13<
    SpiBus,
    gpio::Input<'static, gpio::Gpio25>,
    gpio::Output<'static, gpio::Gpio27>,
    gpio::Output<'static, gpio::Gpio26>,
    Delay,
>;

#[main]
async fn main(spawner: Spawner) {
    esp_alloc::heap_allocator!(50 * 1000);

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);

    let clocks = ClockControl::configure(system.clock_control, CpuClock::Clock80MHz).freeze();
    let mut delay = Delay::new(&clocks);

    esp_println::logger::init_logger(log::LevelFilter::Info);

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timer_group0.timer0);

    // Setup the EPD display, over the SPI bus.
    let io = gpio::Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let cs = gpio::Output::new(io.pins.gpio15, gpio::Level::High);
    let busy = gpio::Input::new(io.pins.gpio25, gpio::Pull::None);
    let rst = gpio::Output::new(io.pins.gpio26, gpio::Level::High);
    let dc = gpio::Output::new(io.pins.gpio27, gpio::Level::Low);

    let spi = make_static!(
        RefCell<Spi>,
        RefCell::new(
            Spi::new(peripherals.SPI2, 8.MHz(), SpiMode::Mode0, &clocks).with_pins(
                Some(io.pins.gpio13), // sclk
                Some(io.pins.gpio14), // mosi
                gpio::NO_PIN,
                gpio::NO_PIN,
            ),
        )
    );

    let mut spi_bus = RefCellDevice::new(&*spi, cs, delay).unwrap();

    let mut epd = EpdDisplay::new(&mut spi_bus, busy, dc, rst, &mut delay, None)
        .expect("EPaper should be present");

    let display = make_static!(EpdBuffer, EpdBuffer::default());
    display.set_rotation(DisplayRotation::Rotate90);

    let font = Font::try_from_bytes(include_bytes!("../Comfortaa-Medium-Latin.ttf")).unwrap();
    let mut display_error = |text: &str| {
        log::info!("{text}");
        draw::text_to_display(display, font.clone(), text);

        epd.update_and_display_frame(&mut spi_bus, display.buffer(), &mut delay)
            .expect("EPaper should accept update/display requests");
    };

    // Setup the WIFI connection.
    let wifi_stack = match wifi::connect(
        &spawner,
        &clocks,
        peripherals.TIMG1,
        peripherals.RNG,
        peripherals.RADIO_CLK,
        peripherals.WIFI,
    )
    .await
    {
        Ok(stack) => stack,
        Err(err) => {
            display_error(&format!("Failed to connect to wifi: {err:?}"));
            coma(peripherals.LPWR);
        }
    };

    // Setup HTTPS client
    let state = make_static!(TcpClientState<1, 8192, 8192>, TcpClientState::new());
    let tcp_client = TcpClient::new(wifi_stack, &*state);
    let dns_socket = DnsSocket::new(wifi_stack);

    let config = reqwless::client::TlsConfig::new(
        const_random::const_random!(u64),
        make_static!([u8; 8192], [0; 8192]),
        make_static!([u8; 8192], [0; 8192]),
        reqwless::client::TlsVerify::None,
    );

    let mut client = HttpClient::new_with_tls(&tcp_client, &dns_socket, config);

    // Start main loop
    let mut prev_text = heapless::String::new();
    let rx_buffer = make_static!([u8; 4096], [0; 4096]);
    loop {
        log::info!("Refreshing front status");
        let text = match simplyplural::fetch_current_front_name(&mut client, rx_buffer).await {
            Ok(text) => text,
            Err(err) => {
                log::info!("{err:?}");

                let mut string = format!("Err: {err:?}");
                string.truncate(32);

                heapless::String::from_str(&string).unwrap()
            }
        };

        if text == prev_text {
            log::info!("Front status has not changed");
        } else {
            draw::clear_display(display);
            draw::text_to_display(display, font.clone(), text.trim_end());

            epd.update_and_display_frame(&mut spi_bus, display.buffer(), &mut delay)
                .expect("EPaper should accept update/display requests");

            prev_text = text;
        }

        Timer::after_secs(60).await;
    }
}
