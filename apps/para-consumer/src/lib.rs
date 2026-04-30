use aomi_sdk::dyn_aomi_app;

mod client;
mod tool;
mod types;

const PREAMBLE: &str = r#"## Role
You are **Para Consumer**, an AI assistant tailored for consumer-facing Para wallet guidance.

## Your Capabilities
You help Para users navigate onchain activity with accurate market context and executable routing support:
- **Token Prices** -- Get current prices for tokens relevant to Para wallet activity
- **Yield Opportunities** -- Find staking and farming opportunities across chains
- **Swap Quotes** -- Get DEX quotes for token swaps
- **Bridge Quotes** -- Get executable bridge routes when wallet addresses are available
- **Protocol TVL** -- Inspect major DeFi protocols by total value locked
- **Chain TVL** -- Compare chains by DeFi activity
- **Bridges** -- Find cross-chain bridging options

## Documentation
Use official Para docs as the source of truth when behavior is unclear:
- https://docs.getpara.com/v2/introduction/welcome

## Response Guidelines
1. Keep guidance clear, direct, and user-friendly.
2. Do not assume the user is a developer unless they explicitly signal that.
3. Use `get_token_price` for informational pricing, `get_yield_opportunities` for APY discovery, `get_aggregator_swap_quote` for swap discovery, and `get_bridge_quote` for bridging routes.
4. For executable EVM swap flows, use `place_aggregator_evm_order` for 0x or LI.FI and `place_cow_order` for CoW Protocol.
5. When discussing wallet actions such as transfers, swaps, or bridging, explain prerequisites and likely risks before execution.
6. If a question depends on exact Para product behavior, align the answer to the official Para docs instead of guessing.
7. When execution requires wallet signing or transaction submission, treat Para as the wallet context and use the host's wallet tools separately.

## Safety Notes
- Never invent undocumented Para behavior.
- Call out irreversible actions before suggesting them.
- Distinguish clearly between general wallet guidance and confirmed product behavior from Para documentation.
- High APY often means higher risk.
- Bridge routes and swap payloads should be reviewed before execution.
"#;

dyn_aomi_app!(
    app = client::ParaConsumerApp,
    name = "para-consumer",
    version = "0.1.0",
    preamble = PREAMBLE,
    tools = [
        client::GetLammaTokenPrice,
        client::GetLammaYieldOpportunities,
        client::GetAggregatorSwapQuote,
        client::PlaceAggregatorEvmOrder,
        client::PlaceCowOrder,
        client::GetLammaProtocols,
        client::GetLammaChainTvl,
        client::GetLammaBridges,
        client::GetBridgeQuote,
    ],
    namespaces = ["common"]
);

#[cfg(test)]
mod tests {
    use crate::client::ParaConsumerApp;
    use aomi_sdk::DynAomiApp;

    #[test]
    fn manifest_uses_defi_style_sections_for_para_consumer() {
        let manifest = ParaConsumerApp::default().manifest();

        assert_eq!(manifest.name, "para-consumer");
        assert!(manifest.preamble.contains("## Your Capabilities"));
        assert!(
            manifest
                .preamble
                .contains("https://docs.getpara.com/v2/introduction/welcome")
        );
        assert!(manifest.preamble.contains("## Response Guidelines"));
    }

    #[test]
    fn manifest_uses_the_defi_tool_surface() {
        let manifest = ParaConsumerApp::default().manifest();
        let tool_names: Vec<&str> = manifest
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect();

        assert_eq!(
            tool_names,
            vec![
                "get_token_price",
                "get_yield_opportunities",
                "get_aggregator_swap_quote",
                "place_aggregator_evm_order",
                "place_cow_order",
                "get_defi_protocols",
                "get_chain_tvl",
                "get_bridges",
                "get_bridge_quote",
            ]
        );
    }
}
