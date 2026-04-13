use super::*;
use polymarket_client_sdk::clob::types::SignatureType;
use serde_json::json;

#[test]
fn public_market_apis_smoke() {
    let client = PolymarketClient::new().expect("client should build");
    let markets = match client.get_markets(&GetMarketsParams {
        limit: Some(1),
        offset: Some(0),
        active: Some(true),
        closed: Some(false),
        archived: Some(false),
        tag: None,
    }) {
        Ok(markets) => markets,
        Err(err) if err.to_ascii_lowercase().contains("request failed") => return,
        Err(err) => panic!("market list request should succeed: {err}"),
    };

    assert!(!markets.is_empty(), "expected at least one active market");

    let first = &markets[0];
    let slug = first
        .slug
        .as_deref()
        .expect("market list response should include slug");
    let condition_id = first
        .condition_id
        .as_deref()
        .expect("market list response should include condition_id");

    let by_slug = client
        .get_market(slug)
        .expect("market lookup by slug should succeed");
    assert_eq!(by_slug.slug.as_deref(), Some(slug));

    let by_condition_id = client
        .get_market(condition_id)
        .expect("market lookup by condition_id should succeed");
    assert_eq!(by_condition_id.condition_id.as_deref(), Some(condition_id));

    let trades = match client.get_trades(&GetTradesParams {
        limit: Some(1),
        offset: Some(0),
        market: None,
        user: None,
        side: None,
    }) {
        Ok(trades) => trades,
        Err(err) if err.to_ascii_lowercase().contains("request failed") => return,
        Err(err) => panic!("trades request should succeed: {err}"),
    };
    assert!(!trades.is_empty(), "expected at least one trade");
}

#[test]
fn default_order_path_matches_docs() {
    let client = PolymarketClient::new().expect("client should build");
    let path = client
        .extract_request_path(&format!("{CLOB_API_BASE}/order"))
        .expect("request path extraction should succeed");

    assert_eq!(path, "/order");
}

#[test]
fn rejects_reusing_order_signature_for_clob_l1_auth() {
    let client = PolymarketClient::new().expect("client should build");
    let shared_signature = "0x4f5ebd67f345143fe72b896c26bc11cc69c44fc8e75f2c4bfa2aa6b51316cf84552633fe49c00e9e43bd3d16d1a7c993095f0f7c8d35e04e72993f2d93c122741c";
    let err = client
        .validate_l1_auth_for_bootstrap(
            &ClobL1Auth {
                address: "0x5D907BEa404e6F821d467314a9cA07663CF64c9B".to_string(),
                signature: shared_signature.to_string(),
                timestamp: PolymarketClient::now_unix_timestamp(),
                nonce: Some("0".to_string()),
            },
            shared_signature,
        )
        .expect_err("should reject reused signature");

    assert!(
        err.contains("cannot reuse the signed order signature"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_stale_clob_l1_timestamp() {
    let client = PolymarketClient::new().expect("client should build");
    let err = client
        .validate_l1_auth_for_bootstrap(
            &ClobL1Auth {
                address: "0x5D907BEa404e6F821d467314a9cA07663CF64c9B".to_string(),
                signature: "0xdeadbeef".to_string(),
                timestamp: "1744329600".to_string(),
                nonce: Some("0".to_string()),
            },
            "0xbeadfeed",
        )
        .expect_err("should reject stale timestamp");

    assert!(
        err.contains("fresh current/server timestamp"),
        "unexpected error: {err}"
    );
}

#[test]
fn sdk_signature_type_defaults_to_proxy() {
    let (signature_type, label) =
        resolve_sdk_signature_type(None).expect("missing signature type should default");
    assert_eq!(signature_type, SignatureType::Proxy);
    assert_eq!(label, "proxy");
}

#[test]
fn sdk_signature_type_accepts_expected_labels() {
    let cases = [
        ("proxy", SignatureType::Proxy, "proxy"),
        ("eoa", SignatureType::Eoa, "eoa"),
        ("gnosis-safe", SignatureType::GnosisSafe, "gnosis-safe"),
        ("safe", SignatureType::GnosisSafe, "gnosis-safe"),
    ];

    for (input, expected, expected_label) in cases {
        let (signature_type, label) = resolve_sdk_signature_type(Some(input))
            .unwrap_or_else(|err| panic!("expected {input} to parse: {err}"));
        assert_eq!(signature_type, expected);
        assert_eq!(label, expected_label);
    }
}

#[test]
fn signed_order_body_uses_api_key_owner_and_sdk_shape() {
    let body = build_signed_order_body(
        &ClobApiCredentials {
            key: "00000000-0000-0000-0000-000000000000".to_string(),
            secret: "secret".to_string(),
            passphrase: "passphrase".to_string(),
        },
        &PreparedPolymarketOrder {
            order: PreparedPolymarketExchangeOrder {
                salt: 42,
                maker: "0x1111111111111111111111111111111111111111".to_string(),
                signer: "0x2222222222222222222222222222222222222222".to_string(),
                taker: "0x0000000000000000000000000000000000000000".to_string(),
                token_id: "123".to_string(),
                maker_amount: "1000000".to_string(),
                taker_amount: "2000000".to_string(),
                expiration: "0".to_string(),
                nonce: "0".to_string(),
                fee_rate_bps: "0".to_string(),
                side: "BUY".to_string(),
                side_index: 0,
                signature_type: 1,
            },
            order_type: "FAK".to_string(),
            post_only: None,
            verifying_contract: "0x4444444444444444444444444444444444444444".to_string(),
        },
        "0xabc123",
    );

    assert_eq!(
        body,
        json!({
            "owner": "00000000-0000-0000-0000-000000000000",
            "orderType": "FAK",
            "order": {
                "salt": 42,
                "maker": "0x1111111111111111111111111111111111111111",
                "signer": "0x2222222222222222222222222222222222222222",
                "taker": "0x0000000000000000000000000000000000000000",
                "tokenId": "123",
                "makerAmount": "1000000",
                "takerAmount": "2000000",
                "expiration": "0",
                "nonce": "0",
                "feeRateBps": "0",
                "side": "BUY",
                "signatureType": 1,
                "signature": "0xabc123",
            }
        })
    );
}
