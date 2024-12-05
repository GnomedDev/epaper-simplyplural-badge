use core::str::FromStr;

use aformat::{aformat, ArrayString, CapStr};
use heapless::String;
use reqwless::request::{Method, RequestBuilder as _};
use serde::{de::Error as _, Deserialize as _};

type DnsSocket<'a> = embassy_net::dns::DnsSocket<'a>;
type TcpClient<'a> = embassy_net::tcp::client::TcpClient<'a, 1, 8192, 8192>;

pub type HttpClient<'a> = reqwless::client::HttpClient<'a, TcpClient<'a>, DnsSocket<'a>>;

fn arraystring_to_heapless<const N1: usize, const N2: usize>(val: ArrayString<N1>) -> String<N2> {
    const { assert!(N2 >= N1, "Cannot fit ArrayString into String") }
    String::from_str(val.as_str()).unwrap()
}

fn filter_characters<'de, const N: usize, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<String<N>, D::Error> {
    let str = serde_cow::CowStr::deserialize(deserializer)?.0;

    let mut out = String::new();
    for char in str.chars() {
        if !char.is_alphabetic() && !['?', ' ', '(', ')', ','].contains(&char) {
            continue;
        }

        if out.push(char).is_err() {
            return Err(D::Error::invalid_length(
                str.len(),
                &aformat!("A string that fit into {N} bytes").as_str(),
            ));
        }
    }

    Ok(out)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SPResponse {
    #[serde(deserialize_with = "filter_characters")]
    front_string: String<32>,
    #[serde(deserialize_with = "filter_characters")]
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
    if !resp.status.is_successful() {
        let err_msg = aformat!("SP Error: {}", resp.status.0);
        return Ok(arraystring_to_heapless(err_msg));
    }

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
