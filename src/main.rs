#![no_std]
#![no_main]
#![warn(rust_2018_idioms)]
#![feature(type_alias_impl_trait, concat_bytes)]

use core::hint::black_box;

use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::{
    clock::ClockControl,
    peripherals::Peripherals,
    prelude::*,
    system::SystemControl,
    timer::{timg::TimerGroup, ErasedTimer, OneShotTimer},
};

mod wifi;

#[macro_export]
macro_rules! make_static {
    ($t:ty, $val:expr) => ($crate::make_static!($t, $val,));
    ($t:ty, $val:expr, $(#[$m:meta])*) => {{
        $(#[$m])*
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        STATIC_CELL.init_with(|| $val)
    }};
}

#[main]
async fn main(spawner: Spawner) {
    esp_alloc::heap_allocator!(50 * 1000);

    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);

    let clocks = ClockControl::max(system.clock_control).freeze();

    esp_println::logger::init_logger(log::LevelFilter::Info);

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks, None);
    esp_hal_embassy::init(
        &clocks,
        make_static!(
            [OneShotTimer<ErasedTimer>; 1],
            [OneShotTimer::new(timer_group0.timer0.into())]
        ),
    );

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
            log::info!("Failed to connect to wifi: {err:?}");
            return;
        }
    };

    black_box(([0_u64; 16000], wifi_stack));
}
