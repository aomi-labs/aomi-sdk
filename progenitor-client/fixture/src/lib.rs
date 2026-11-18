//! Clients that `build.rs` generates from `specs/` with the canonical
//! generator. Building this crate for `wasm32-wasip2` proves the generated
//! code compiles against `aomi_sdk::http` in a guest.

#[allow(clippy::all, dead_code, unused_imports)]
pub mod across {
    include!(concat!(env!("OUT_DIR"), "/across.rs"));
}

#[allow(clippy::all, dead_code, unused_imports)]
pub mod verbs {
    include!(concat!(env!("OUT_DIR"), "/verbs.rs"));
}

#[cfg(test)]
mod tests {
    use aomi_progenitor_client::{Error, ResponseValue};
    use aomi_sdk::testing::reply;

    use super::{across, verbs};

    /// Type-level check of the generated signatures: positional arguments,
    /// typed bodies and results, the record readable before `into_inner`, and
    /// `?` into a tool's `aomi_sdk::Error`. Native HTTP has no mock
    /// transport, so this is compiled, not run.
    #[allow(dead_code)]
    async fn calls(a: &across::Client, v: &verbs::Client) -> aomi_sdk::Result<Vec<u32>> {
        let limits = a.get_limits(10, "0xin", 1, "0xout").await?;
        let routes = a.get_available_routes(None, None, Some(1), None).await?;
        let order = verbs::types::Order {
            symbol: "BTC".into(),
            qty: "1".into(),
            created_at: None,
        };
        let replaced = match v.replace_order("o1", "key", None, &order).await {
            Err(Error::ErrorResponse(problem)) => {
                return Err(aomi_sdk::err("rejected", &problem.code));
            }
            other => other?,
        };
        let created = v.create_order(&order).await?;
        let renamed = v.rename_order("o1", "new".into()).await?;
        let cancelled = v
            .cancel_order("o1", Some(verbs::types::CancelOrderSide::Buy))
            .await?;
        let csv: ResponseValue<Vec<u8>> = v.export_csv(7, Some(&vec!["a".into()])).await?;
        let uploaded = v.upload(b"bytes".to_vec()).await?;
        let _: (across::types::AcrossLimits, Vec<across::types::AcrossRoute>) =
            (limits.into_inner(), routes.into_inner());
        Ok(vec![
            replaced.record()?,
            created.record()?,
            renamed.record()?,
            cancelled.record()?,
            csv.record()?,
            uploaded.record()?,
        ])
    }

    #[test]
    fn generated_types_decode_a_recorded_reply() {
        let body = r#"{"minDeposit":"1","maxDeposit":"9","maxDepositInstant":"5",
            "maxDepositShortDelay":"7","recommendedDepositInstant":"3"}"#;
        let limits = ResponseValue::<across::types::AcrossLimits>::from_response::<()>(reply(
            200,
            &[("aomi-record", "12")],
            body,
        ))
        .unwrap();
        assert_eq!(limits.record().unwrap(), 12);
        assert_eq!(limits.into_inner().max_deposit, "9");

        let problem = ResponseValue::<verbs::types::Problem>::from_response::<()>(reply(
            404,
            &[("aomi-record", "13")],
            r#"{"code":"unknown_order"}"#,
        ))
        .unwrap();
        let e = Error::ErrorResponse(problem);
        assert_eq!(e.status(), Some(404));
        assert_eq!(aomi_sdk::Error::from(e).code, "http_status");
    }
}
