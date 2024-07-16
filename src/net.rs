use embassy_net::{tcp::client::TcpClientState, Stack};
use esp_hal::peripherals::RSA;
use reqwless::request::Method;

use crate::make_static;

type WifiDriver = esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>;

type DnsSocket = embassy_net::dns::DnsSocket<'static, WifiDriver>;
type TcpClient = embassy_net::tcp::client::TcpClient<'static, WifiDriver, 1, 4096, 4096>;

pub type HttpClient = reqwless::client::HttpClient<'static, TcpClient, DnsSocket>;

static CERT: &[u8] = &*concat_bytes!(include_bytes!("../www-google-com-chain.pem"), b'\0');

pub fn init_http_client(wifi_stack: &'static Stack<WifiDriver>, rsa: RSA) -> HttpClient {
    let state = make_static!(TcpClientState<1, 4096, 4096>, TcpClientState::new());
    let tcp_client = make_static!(TcpClient, TcpClient::new(wifi_stack, &*state));
    let dns_socket = make_static!(DnsSocket, DnsSocket::new(wifi_stack));

    let config = reqwless::client::TlsConfig::new(
        reqwless::TlsVersion::Tls1_3,
        reqwless::Certificates {
            ca_chain: Some(reqwless::X509::pem(CERT).unwrap()),
            ..Default::default()
        },
        Some(make_static!(RSA, rsa)), // Will use hardware acceleration
    );

    HttpClient::new_with_tls(&*tcp_client, &*dns_socket, config)
}

#[allow(clippy::large_futures)]
pub async fn perform_request(http: &mut HttpClient) -> Result<(), reqwless::Error> {
    let url = "https://google.com";
    log::info!("Connecting to {url}");
    let mut request = http.request(Method::GET, url).await?;

    log::info!("Sending request");
    let mut rx_buf = [0; 1024];
    let resp = request.send(&mut rx_buf).await?;

    log::info!("Reading body");
    let body = resp.body().read_to_end().await?;

    log::info!("{}", core::str::from_utf8(body).unwrap());
    Ok(())
}
