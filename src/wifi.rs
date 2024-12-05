use embassy_executor::Spawner;
use embassy_net::{Config, DhcpConfig, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    peripherals::{RADIO_CLK, RNG, TIMG1, WIFI},
    rng::Rng,
};
use esp_wifi::{
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiError, WifiEvent,
        WifiStaDevice, WifiState,
    },
    EspWifiController,
};

use crate::make_static;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

pub async fn connect(
    spawner: &Spawner,
    timg1: TIMG1,
    rng: RNG,
    radio_clk: RADIO_CLK,
    wifi: WIFI,
) -> Result<Stack<'static>, WifiError> {
    let timer_group1 = esp_hal::timer::timg::TimerGroup::new(timg1);

    log::info!("Initialising WIFI");
    let init = esp_wifi::init(timer_group1.timer0, Rng::new(rng), radio_clk)
        .expect("WIFI should not fail initialization");

    log::info!("Creating network interface and wifi stack");
    let (wifi_iface, controller) = esp_wifi::wifi::new_with_mode(
        make_static!(EspWifiController<'static>, init),
        wifi,
        WifiStaDevice,
    )?;

    let resources = make_static!(StackResources::<3>, StackResources::<3>::new());
    let (stack, runner) = embassy_net::new(
        wifi_iface,
        Config::dhcpv4(DhcpConfig::default()),
        &mut *resources,
        const_random::const_random!(u64),
    );

    log::info!("Spawning background wifi tasks");
    spawner.spawn(connection(controller)).unwrap();
    spawner.spawn(net_task(runner)).unwrap();

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
        if esp_wifi::wifi::wifi_state() == WifiState::StaConnected {
            log::info!("[ConnTask] Connected to wifi");

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
            controller.start_async().await.unwrap();
        }

        log::info!("[ConnTask] Connecting to wifi");
        if let Err(e) = controller.connect_async().await {
            log::info!("[ConnTask] Failed to connect: {e:?}");
            Timer::after(Duration::from_millis(5000)).await;
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut stack: embassy_net::Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await;
}
