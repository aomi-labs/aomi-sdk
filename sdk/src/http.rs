//! Outbound HTTP, reqwest-shaped, over `wstd` and `wasi:http`.
//!
//! The host allows only the hosts in `[network] hosts`, injects declared
//! secret slots, refuses redirects and records every reply. A reply's record
//! number, the evidence a prepared transaction may cite, is
//! [`Response::record`].

use serde::Serialize;
use serde::de::DeserializeOwned;
use wstd::http::{Body, Client, Method, Request};

pub use wstd::http::HeaderMap;

use crate::{Error, Result, err};

/// The response header the host sets to its record number for the reply.
const RECORD_HEADER: &str = "aomi-record";

pub fn get(url: impl Into<String>) -> RequestBuilder {
    RequestBuilder::new(Method::GET, url.into())
}

pub fn post(url: impl Into<String>) -> RequestBuilder {
    RequestBuilder::new(Method::POST, url.into())
}

/// A request being built. A bad query or body is kept and returned by `send`.
#[derive(Debug)]
pub struct RequestBuilder {
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    error: Option<Error>,
}

impl RequestBuilder {
    fn new(method: Method, url: String) -> Self {
        Self {
            method,
            url,
            headers: Vec::new(),
            body: Vec::new(),
            error: None,
        }
    }

    /// Appends URL-encoded pairs or a struct to the query string.
    pub fn query<T: Serialize + ?Sized>(mut self, query: &T) -> Self {
        match serde_urlencoded::to_string(query) {
            Ok(q) if q.is_empty() => {}
            Ok(q) => {
                self.url
                    .push(if self.url.contains('?') { '&' } else { '?' });
                self.url.push_str(&q);
            }
            Err(e) => self.fail(err("invalid_query", e.to_string())),
        }
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// A JSON body with `content-type: application/json`.
    pub fn json<T: Serialize + ?Sized>(mut self, body: &T) -> Self {
        match serde_json::to_vec(body) {
            Ok(body) => {
                self.body = body;
                self.header("content-type", "application/json")
            }
            Err(e) => {
                self.fail(e.into());
                self
            }
        }
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// Sends the request and reads the whole reply.
    pub async fn send(self) -> Result<Response> {
        if let Some(e) = self.error {
            return Err(e);
        }
        let mut request = Request::builder().method(self.method).uri(&self.url);
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        let body = if self.body.is_empty() {
            Body::empty()
        } else {
            self.body.into()
        };
        let request = request
            .body(body)
            .map_err(|e| err("http_invalid_request", e.to_string()))?;
        let mut response = Client::new().send(request).await.map_err(failed)?;
        let body = response
            .body_mut()
            .contents()
            .await
            .map_err(failed)?
            .to_vec();
        Ok(Response {
            status: response.status().as_u16(),
            headers: response.headers().clone(),
            body,
        })
    }

    fn fail(&mut self, e: Error) {
        self.error.get_or_insert(e);
    }
}

fn failed(e: wstd::http::Error) -> Error {
    err("http_failed", format!("{e:#}"))
}

/// A whole HTTP reply.
#[derive(Debug)]
pub struct Response {
    status: u16,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Response {
    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    pub fn text(&self) -> Result<&str> {
        std::str::from_utf8(&self.body).map_err(|e| err("invalid_utf8", e.to_string()))
    }

    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_slice(&self.body)?)
    }

    /// The host's record number for this reply, from its `aomi-record` header.
    pub fn record(&self) -> Result<u32> {
        self.headers
            .get(RECORD_HEADER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok())
            .ok_or_else(|| err("evidence_missing", "the reply has no aomi-record header"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(record: Option<&str>) -> Response {
        let mut headers = HeaderMap::new();
        if let Some(record) = record {
            headers.insert(RECORD_HEADER, record.parse().unwrap());
        }
        Response {
            status: 200,
            headers,
            body: br#"{"ok":true}"#.to_vec(),
        }
    }

    #[test]
    fn record_reads_the_host_header() {
        assert_eq!(reply(Some("3")).record().unwrap(), 3);
        assert_eq!(reply(None).record().unwrap_err().code, "evidence_missing");
        assert_eq!(
            reply(Some("x")).record().unwrap_err().code,
            "evidence_missing"
        );
    }

    #[test]
    fn query_appends_encoded_pairs() {
        let request = get("https://api.jup.ag/price/v3?a=1").query(&[("ids", "So1,EPj x")]);
        assert_eq!(
            request.url,
            "https://api.jup.ag/price/v3?a=1&ids=So1%2CEPj+x"
        );
        let body: serde_json::Value = reply(None).json().unwrap();
        assert_eq!(body["ok"], true);
    }
}
