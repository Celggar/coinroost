//! How a tool calls the API: a real request through the API router, carrying the caller's bearer, and how it reads
//! names and amounts the way a person (or a model) gives them.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::request::Parts;
use axum::http::{Method, Request, header};
use rmcp::ErrorData;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::auth;
use crate::validate::minor_digits;

/// Why a tool could not do what was asked: the API's problem detail, or a reading of the arguments.
pub struct Failure(pub String);

impl From<String> for Failure {
    fn from(message: String) -> Failure {
        Failure(message)
    }
}

/// A tool's answer: the JSON on success; the reason as an error result, so the model can react to it.
#[allow(clippy::unnecessary_wraps)]
pub fn outcome(result: Result<Value, Failure>) -> Result<CallToolResult, ErrorData> {
    Ok(match result {
        Ok(value) => CallToolResult::success(vec![ContentBlock::text(value.to_string())]),
        Err(Failure(reason)) => CallToolResult::error(vec![ContentBlock::text(reason)]),
    })
}

/// The household's things a tool can name.
#[derive(Clone, Copy)]
pub enum Named {
    Account,
    Category,
    Payee,
    Rule,
}

impl Named {
    fn path(self) -> &'static str {
        match self {
            Named::Account => "/api/v1/accounts",
            Named::Category => "/api/v1/categories",
            Named::Payee => "/api/v1/payees",
            Named::Rule => "/api/v1/rules",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Named::Account => "account",
            Named::Category => "category",
            Named::Payee => "payee",
            Named::Rule => "rule",
        }
    }
}

pub struct Client<'a> {
    api: &'a Router,
    bearer: String,
}

impl<'a> Client<'a> {
    pub fn new(api: &'a Router, parts: &Parts) -> Result<Client<'a>, Failure> {
        let bearer = auth::bearer(&parts.headers).ok_or_else(|| Failure("no bearer token".to_owned()))?;
        Ok(Client { api, bearer })
    }

    /// Sends one request to the API; a non-2xx answer becomes a failure carrying the problem's detail.
    pub async fn send(
        &self,
        method: Method,
        uri: &str,
        body: Option<&Value>,
        if_match: Option<&str>,
    ) -> Result<Value, Failure> {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.bearer));
        if let Some(tag) = if_match {
            request = request.header(header::IF_MATCH, tag);
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .map_err(|e| Failure(e.to_string()))?;
        let Ok(response) = self.api.clone().oneshot(request).await;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 16 << 20)
            .await
            .map_err(|e| Failure(e.to_string()))?;
        let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        if status.is_success() {
            return Ok(value);
        }
        let detail = value["detail"]
            .as_str()
            .or(value["title"].as_str())
            .unwrap_or("no detail");
        Err(Failure(format!("the API answered {}: {detail}", status.as_u16())))
    }

    pub async fn get(&self, uri: &str) -> Result<Value, Failure> {
        self.send(Method::GET, uri, None, None).await
    }

    /// The household's items of a kind, as the API lists them.
    pub async fn items(&self, kind: Named) -> Result<Vec<Value>, Failure> {
        let list = self.get(kind.path()).await?;
        Ok(list["items"].as_array().cloned().unwrap_or_default())
    }

    /// The item `given` names: its id, or its name in any case. An unknown or ambiguous name lists the choices.
    pub async fn resolve(&self, kind: Named, given: &str) -> Result<Value, Failure> {
        let items = self.items(kind).await?;
        let wanted = given.trim();
        if let Some(item) = items.iter().find(|i| i["id"] == wanted) {
            return Ok(item.clone());
        }
        let named: Vec<&Value> = items
            .iter()
            .filter(|i| {
                i["name"]
                    .as_str()
                    .is_some_and(|n| n.to_lowercase() == wanted.to_lowercase())
            })
            .collect();
        match named.as_slice() {
            [item] => Ok((*item).clone()),
            [] => {
                let choices: Vec<&str> = items.iter().filter_map(|i| i["name"].as_str()).collect();
                Err(Failure(format!(
                    "no {} named {wanted:?}; the household has: {}",
                    kind.label(),
                    if choices.is_empty() {
                        "none".to_owned()
                    } else {
                        choices.join(", ")
                    }
                )))
            }
            many => {
                let ids: Vec<&str> = many.iter().filter_map(|i| i["id"].as_str()).collect();
                Err(Failure(format!(
                    "{} items are named {wanted:?}: give the {} id, one of {}",
                    many.len(),
                    kind.label(),
                    ids.join(", ")
                )))
            }
        }
    }

    pub async fn resolve_id(&self, kind: Named, given: &str) -> Result<String, Failure> {
        let item = self.resolve(kind, given).await?;
        Ok(item["id"].as_str().unwrap_or_default().to_owned())
    }

    /// Id → name, to show names instead of ids.
    pub async fn names(&self, kind: Named) -> Result<std::collections::HashMap<String, String>, Failure> {
        Ok(self
            .items(kind)
            .await?
            .iter()
            .filter_map(|i| Some((i["id"].as_str()?.to_owned(), i["name"].as_str()?.to_owned())))
            .collect())
    }
}

/// The id `given` names, or null when nothing (or an empty text) is given: how a tool clears a field.
pub async fn optional(client: &Client<'_>, kind: Named, given: Option<&str>) -> Result<Value, Failure> {
    match given.map(str::trim) {
        None | Some("") => Ok(Value::Null),
        Some(given) => Ok(json!(client.resolve_id(kind, given).await?)),
    }
}

/// A path with its query, the values encoded; `None` values are left out.
pub fn with_query(path: &str, pairs: &[(&str, Option<String>)]) -> String {
    let present: Vec<(&str, &str)> = pairs
        .iter()
        .filter_map(|(k, v)| v.as_deref().map(|v| (*k, v)))
        .collect();
    match serde_urlencoded::to_string(present) {
        Ok(query) if !query.is_empty() => format!("{path}?{query}"),
        _ => path.to_owned(),
    }
}

/// A positive decimal such as "12.34" in minor units of a currency with `digits` decimals. More decimals than the
/// currency has is an error, never rounded.
pub fn minor_of(text: &str, digits: u16) -> Result<i64, Failure> {
    let text = text.trim();
    let digits = usize::from(digits);
    let refuse = || {
        Failure(format!(
            "an amount is a positive decimal such as 12.34, with at most {digits} decimals"
        ))
    };
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let digit_only = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() || !digit_only(whole) || !digit_only(fraction) || fraction.len() > digits {
        return Err(refuse());
    }
    if text.ends_with('.') {
        return Err(refuse());
    }
    let minor: i64 = format!("{whole}{fraction:0<digits$}")
        .parse()
        .map_err(|_| refuse())?;
    if minor == 0 {
        return Err(Failure("an amount must be above zero".to_owned()));
    }
    Ok(minor)
}

/// A positive decimal in minor units of `currency`.
pub fn to_minor(text: &str, currency: &str) -> Result<i64, Failure> {
    let digits = minor_digits(currency).ok_or_else(|| Failure(format!("{currency} has no minor unit")))?;
    minor_of(text, digits)
}

/// Minor units as a decimal string of the currency: -4550 USD → "-45.50".
pub fn decimal(minor: i64, currency: &str) -> String {
    let digits = u32::from(minor_digits(currency).unwrap_or(2));
    let scale = 10u64.pow(digits);
    let sign = if minor < 0 { "-" } else { "" };
    let (whole, part) = (minor.unsigned_abs() / scale, minor.unsigned_abs() % scale);
    if digits == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{part:0width$}", width = digits as usize)
    }
}
