use esp_backtrace as _;
use esp_hal::{
    clock::Clocks,
    peripherals::{RADIO_CLK, RNG, TIMG1, WIFI},
    rng::Rng,
};
use esp_wifi::{
    current_millis, initialize,
    wifi::{
        utils::create_network_interface, ClientConfiguration, Configuration, WifiError,
        WifiStaDevice,
    },
    wifi_interface::WifiStack,
    EspWifiInitFor,
};

use smoltcp::iface::SocketStorage;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

pub fn connect<'a>(
    clocks: &Clocks<'_>,
    timg1: TIMG1,
    rng: RNG,
    radio_clk: RADIO_CLK,
    wifi: WIFI,

    socket_set: &'a mut [SocketStorage<'a>; 3],
) -> Result<WifiStack<'a, WifiStaDevice>, WifiError> {
    let timer = esp_hal::timer::timg::TimerGroup::new(timg1, clocks, None).timer0;

    log::info!("Initialising WIFI");
    let init = initialize(
        EspWifiInitFor::Wifi,
        timer,
        Rng::new(rng),
        radio_clk,
        clocks,
    )
    .expect("WIFI should not fail initialization");

    log::info!("Creating network interface and wifi stack");
    let (iface, device, mut controller, sockets) =
        create_network_interface(&init, wifi, WifiStaDevice, socket_set)?;
    let wifi_stack = WifiStack::new(iface, device, sockets, current_millis);

    controller.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        password: PASSWORD.try_into().unwrap(),
        ..Default::default()
    }))?;

    log::info!("Starting WIFI controller");
    controller.start()?;
    log::info!("Connecting WIFI controller");
    controller.connect()?;

    log::info!("Waiting to connect to WIFI");
    while !controller.is_connected()? {
        // Interrupt will trigger to set is_connected to true, no need to work.
    }

    log::info!("Waiting to get an ip address");
    let ip_info = loop {
        if let Ok(ip_info) = wifi_stack.get_ip_info() {
            break ip_info;
        }

        wifi_stack.work();
    };

    log::info!("Connected and got an IP: {ip_info:?}");
    Ok(wifi_stack)
}
