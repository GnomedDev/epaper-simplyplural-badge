use reqwless::request::Method;

pub type WifiDriver = esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>;

pub type DnsSocket = embassy_net::dns::DnsSocket<'static, WifiDriver>;
pub type TcpClient = embassy_net::tcp::client::TcpClient<'static, WifiDriver, 1, 4096, 4096>;

pub type HttpClient = reqwless::client::HttpClient<'static, TcpClient, DnsSocket>;

pub static CERT: &[u8] = &*concat_bytes!(include_bytes!("../www-google-com-chain.pem"), b'\0');

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
