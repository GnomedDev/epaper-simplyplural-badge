use core::str::FromStr;

use aformat::{aformat, CapStr};
use heapless::String;
use reqwless::request::{Method, RequestBuilder as _};

type WifiDriver = esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>;

type DnsSocket<'a> = embassy_net::dns::DnsSocket<'a, WifiDriver>;
type TcpClient<'a> = embassy_net::tcp::client::TcpClient<'a, WifiDriver, 1, 8192, 8192>;

pub type HttpClient<'a> = reqwless::client::HttpClient<'a, TcpClient<'a>, DnsSocket<'a>>;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SPResponse {
    front_string: String<32>,
    custom_front_string: String<32>,
}

pub async fn fetch_current_front_name(
    http: &mut HttpClient<'_>,
    rx_buffer: &mut [u8],
) -> Result<String<32>, reqwless::Error> {
    let url = aformat!(
        "https://api.apparyllis.com/v1/friend/{}/getFrontValue",
        CapStr::<32>(env!("SP_ID"))
    );

    let headers = [("Authorization", env!("SP_KEY"))];
    let mut request = http.request(Method::GET, &url).await?.headers(&headers);

    let resp = request.send(rx_buffer).await?;
    let body = resp.body().read_to_end().await?;

    let resp_json: SPResponse = serde_json::from_slice(body).unwrap();
    let front_status = match (
        resp_json.front_string.is_empty(),
        resp_json.custom_front_string.is_empty(),
    ) {
        (true, true) => String::from_str("Front is Empty").unwrap(),
        (true, false) => resp_json.custom_front_string,
        (false, _) => resp_json.front_string,
    };

    Ok(front_status)
}
