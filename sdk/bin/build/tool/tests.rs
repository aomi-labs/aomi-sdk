use super::*;
use crate::client::Mode;

const SPEC: &str = r#"
openapi: 3.0.3
info: { title: demo, version: "1" }
servers: [{ url: "https://api.demo.test/v1" }]
paths:
  /prices/{coin}:
    get:
      operationId: getPrice
      summary: Price of one coin
      parameters:
        - { name: coin, in: path, required: true, schema: { type: string }, description: Coin id }
        - { name: days, in: query, required: false, schema: { type: integer, format: int32 } }
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema: { type: object, additionalProperties: true }
"#;

#[test]
fn tool_layer_targets_the_guest_runtime() {
    let spec: openapiv3::OpenAPI = serde_yaml::from_str(SPEC).unwrap();
    let ops = collect_ops(&spec);

    let tool = render::tool_rs("demo", &ops, Mode::Shared);
    assert!(!tool.contains("tokio") && !tool.contains("DynAomiTool"));
    let body = tool.split_once("// GET").unwrap().1;
    assert_eq!(
        body,
        r#" /prices/{coin}
pub(crate) struct GetPrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPriceArgs {
    /// Coin id
    pub coin: String,
    #[serde(default)]
    pub days: Option<i32>,
}

impl Tool for GetPrice {
    const NAME: &'static str = "get_price";
    const DESCRIPTION: &'static str = "Price of one coin";
    type Args = GetPriceArgs;

    async fn run(args: Self::Args, _ctx: &Ctx) -> Result<Outcome> {
        let client = GenClient::new("https://api.demo.test/v1");
        let reply = client
            .get_price(args.coin.as_str(), args.days)
            .await
            .map_err(|e| err("request_failed", format!("[demo] get_price: {e}")))?;
        // Reply: loose JSON (`additionalProperties: true`); tighten the spec with `aomi-build tighten-spec`.
        Ok(Outcome::data(reply.into_inner()))
    }
}

"#
    );
    assert!(tool.contains("use demo_client::Client as GenClient;"));
    assert!(
        render::lib_rs("demo", &ops, Mode::Shared).contains("aomi_sdk::package!(tool::GetPrice);")
    );
    assert!(render::aomi_toml("demo", &ops).contains("hosts = [\"api.demo.test\"]"));
}
