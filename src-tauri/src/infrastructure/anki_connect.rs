//! AnkiConnect adapter — implements the [`AnkiGateway`] port over local HTTP.
//!
//! AnkiConnect (<https://git.sr.ht/~foosoft/anki-connect>) listens on
//! `127.0.0.1:8765` and answers JSON-RPC-style envelopes: request
//! `{action, version, params}`, response `{result, error}`. Per ADR 0004 this
//! adapter is a due-count reader only (`findCards` + `is:due`).

use crate::domain::anki::{AnkiError, AnkiGateway};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

/// Default AnkiConnect endpoint (local Anki desktop).
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8765";

/// Single attempt, bounded wait: the due total is cosmetic in this slice, so
/// an unresponsive Anki must never stall the queue (no retries).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Anki search syntax for cards due for review, across every deck.
const DUE_QUERY: &str = "is:due";

/// The AnkiConnect API level this adapter speaks.
const API_VERSION: u8 = 6;

/// [`AnkiGateway`] implementation backed by AnkiConnect's HTTP API.
pub struct AnkiConnectGateway {
    base_url: String,
    http: reqwest::Client,
}

impl AnkiConnectGateway {
    /// Builds a gateway against the given AnkiConnect base URL (e.g.
    /// `http://127.0.0.1:8765`) so settings or tests can point it elsewhere.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client with static configuration always builds"),
        }
    }
}

impl Default for AnkiConnectGateway {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

/// AnkiConnect always answers `{result, error}`, both possibly null.
#[derive(Debug, Deserialize)]
struct AnkiConnectResponse {
    #[serde(default)]
    result: serde_json::Value,
    #[serde(default)]
    error: Option<String>,
}

impl AnkiGateway for AnkiConnectGateway {
    async fn due_total(&self) -> Result<u64, AnkiError> {
        let body = json!({
            "action": "findCards",
            "version": API_VERSION,
            "params": { "query": DUE_QUERY },
        });

        // Transport failure (refused, timeout, reset) means Anki is not there.
        let response = self
            .http
            .post(&self.base_url)
            .json(&body)
            .send()
            .await
            .map_err(|_| AnkiError::Offline)?;

        if !response.status().is_success() {
            return Err(AnkiError::BadResponse(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let text = response.text().await.map_err(|_| AnkiError::Offline)?;
        let payload: AnkiConnectResponse = serde_json::from_str(&text)
            .map_err(|err| AnkiError::BadResponse(format!("not an AnkiConnect payload: {err}")))?;

        if let Some(err) = payload.error {
            return Err(AnkiError::BadResponse(err));
        }

        let card_ids: Vec<serde_json::Value> = serde_json::from_value(payload.result)
            .map_err(|_| AnkiError::BadResponse("findCards result is not a list".into()))?;

        Ok(card_ids.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn gateway(uri: &str) -> AnkiConnectGateway {
        AnkiConnectGateway::new(uri)
    }

    #[tokio::test]
    async fn counts_cards_in_the_is_due_result() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_partial_json(json!({
                "action": "findCards",
                "version": 6,
                "params": { "query": "is:due" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "result": [1494723142483u64, 1494703460437u64, 1494703479525u64],
                "error": null
            })))
            .expect(1)
            .mount(&server)
            .await;

        let total = gateway(&server.uri()).due_total().await;

        assert_eq!(total, Ok(3));
    }

    #[tokio::test]
    async fn anki_error_payload_maps_to_bad_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "result": null,
                "error": "collection is not open"
            })))
            .mount(&server)
            .await;

        let total = gateway(&server.uri()).due_total().await;

        assert_eq!(
            total,
            Err(AnkiError::BadResponse("collection is not open".into()))
        );
    }

    #[tokio::test]
    async fn connection_refused_maps_to_offline() {
        // Reserve an ephemeral port and release it: nothing is listening there.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let total = gateway(&format!("http://127.0.0.1:{port}"))
            .due_total()
            .await;

        assert_eq!(total, Err(AnkiError::Offline));
    }

    #[tokio::test]
    async fn malformed_result_payload_maps_to_bad_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "result": 42,
                "error": null
            })))
            .mount(&server)
            .await;

        let total = gateway(&server.uri()).due_total().await;

        assert!(matches!(total, Err(AnkiError::BadResponse(_))));
    }

    #[tokio::test]
    async fn http_error_status_maps_to_bad_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let total = gateway(&server.uri()).due_total().await;

        assert!(matches!(total, Err(AnkiError::BadResponse(_))));
    }

    /// Manual smoke against a real desktop Anki with AnkiConnect enabled.
    /// Run with: `cargo test -- --ignored`
    #[tokio::test]
    #[ignore = "needs a local Anki with AnkiConnect on 127.0.0.1:8765"]
    async fn real_anki_due_total() {
        let total = AnkiConnectGateway::default().due_total().await;
        println!("Anki due total: {total:?}");
        assert!(total.is_ok());
    }
}
