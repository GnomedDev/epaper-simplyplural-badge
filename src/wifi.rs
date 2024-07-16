use embassy_executor::Spawner;
use embassy_net::{Config, DhcpConfig, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    clock::Clocks,
    peripherals::{RADIO_CLK, RNG, TIMG1, WIFI},
    rng::Rng,
    timer::PeriodicTimer,
};
use esp_wifi::{
    initialize,
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiError, WifiEvent,
        WifiStaDevice, WifiState,
    },
    EspWifiInitFor,
};

use crate::make_static;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

pub async fn connect(
    spawner: &Spawner,
    clocks: &Clocks<'_>,
    timg1: TIMG1,
    rng: RNG,
    radio_clk: RADIO_CLK,
    wifi: WIFI,
) -> Result<&'static Stack<WifiDevice<'static, WifiStaDevice>>, WifiError> {
    let timer = esp_hal::timer::timg::TimerGroup::new(timg1, clocks, None).timer0;

    log::info!("Initialising WIFI");
    let init = initialize(
        EspWifiInitFor::Wifi,
        PeriodicTimer::new(timer.into()),
        Rng::new(rng),
        radio_clk,
        clocks,
    )
    .expect("WIFI should not fail initialization");

    log::info!("Creating network interface and wifi stack");
    let (wifi_iface, controller) = esp_wifi::wifi::new_with_mode(&init, wifi, WifiStaDevice)?;

    let resources = make_static!(StackResources::<3>, StackResources::<3>::new());
    let stack = &*make_static!(
        Stack<WifiDevice<'_, WifiStaDevice>>,
        Stack::new(
            wifi_iface,
            Config::dhcpv4(DhcpConfig::default()),
            resources,
            1234
        )
    );

    log::info!("Spawning background wifi tasks");
    spawner.spawn(connection(controller)).unwrap();
    spawner.spawn(net_task(stack)).unwrap();

    log::info!("Waiting for a WIFI connection and IP");
    loop {
        if let Some(config) = stack.config_v4() {
            log::info!("Got connection: {config:?}");
            break;
        }

        Timer::after(Duration::from_millis(500)).await;
    }

    Ok(stack)
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    log::info!("[ConnTask] Started");
    loop {
        if let WifiState::StaConnected = esp_wifi::wifi::get_wifi_state() {
            log::info!("[ConnTask] Connected to WIFI!");

            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await;
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });

            log::info!("[ConnTask] Setting configuration to: {client_config:?}");
            controller.set_configuration(&client_config).unwrap();

            log::info!("[ConnTask] Starting wifi");
            controller.start().await.unwrap();
        }

        log::info!("[ConnTask] Connecting to wifi");
        if let Err(e) = controller.connect().await {
            log::info!("[ConnTask] Failed to connect: {e:?}");
            Timer::after(Duration::from_millis(5000)).await;
        }
    }
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await;
}
