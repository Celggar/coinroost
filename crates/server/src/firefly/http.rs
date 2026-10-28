//! A plain-HTTP client for Firefly's API that can only send GET requests: the read-only promise of the migration
//! lives in this type, which has no way to send anything else.

use std::time::Duration;

use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::{Request, StatusCode, Uri, header};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

/// How many times a request is sent when the connection fails, waiting twice as long each time from one second:
/// after a boot, the network name of Firefly may resolve only some seconds later.
const ATTEMPTS: u32 = 5;

pub struct Firefly {
    client: Client<HttpConnector, Empty<Bytes>>,
    base: String,
    bearer: String,
}

impl Firefly {
    pub fn new(url: &str, token: &str) -> Result<Firefly, String> {
        let base = url.trim().trim_end_matches('/');
        if !base.starts_with("http://") {
            return Err(
                "FIREFLY_URL must be an http:// address (Firefly on your network, or a local tunnel to \
                        it): this client does not speak TLS"
                    .to_owned(),
            );
        }
        base.parse::<Uri>().map_err(|e| format!("FIREFLY_URL: {e}"))?;
        Ok(Firefly {
            client: Client::builder(TokioExecutor::new()).build_http(),
            base: base.to_owned(),
            bearer: format!("Bearer {}", token.trim()),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// GETs `path_and_query`: the status and the body.
    pub async fn get(&self, path_and_query: &str) -> Result<(StatusCode, Bytes), String> {
        let uri: Uri = format!("{}{path_and_query}", self.base)
            .parse()
            .map_err(|e| format!("{path_and_query}: {e}"))?;
        let mut wait = Duration::from_secs(1);
        let mut attempt = 1;
        loop {
            let request = Request::get(uri.clone())
                .header(header::ACCEPT, "application/json")
                .header(header::AUTHORIZATION, &self.bearer)
                .body(Empty::new())
                .map_err(|e| e.to_string())?;
            match self.client.request(request).await {
                Ok(response) => {
                    let status = response.status();
                    let body = response.into_body().collect().await;
                    let body = body.map_err(|e| format!("{path_and_query}: {e}"))?.to_bytes();
                    return Ok((status, body));
                }
                Err(e) if attempt < ATTEMPTS => {
                    eprintln!("{path_and_query}: {e}; trying again in {} s", wait.as_secs());
                    tokio::time::sleep(wait).await;
                    wait *= 2;
                    attempt += 1;
                }
                Err(e) => return Err(format!("{path_and_query}: {e} (after {ATTEMPTS} attempts)")),
            }
        }
    }
}
