use chrono::Local;
use std::sync::LazyLock;

mod execution;
mod http;
mod intent;
mod market;
mod models;
#[cfg(test)]
mod tests;
mod validation;

pub(crate) use crate::tool::*;
pub(crate) use execution::*;
pub(crate) use http::*;
pub(crate) use intent::*;
pub(crate) use market::*;
pub(crate) use models::*;
pub(crate) use validation::*;

type SdkAuthedClobClient = polymarket_client_sdk::clob::Client<
    polymarket_client_sdk::auth::state::Authenticated<polymarket_client_sdk::auth::Normal>,
>;

pub(crate) static TOKIO_RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime for polymarket")
});

pub(crate) fn build_preamble() -> String {
    let now = Local::now();
    format!(
        r#"## Role
You specialize in Polymarket prediction markets — discovering markets, analyzing trends, and placing trades.

## Current Date
Today is {} ({}). Use this exact date when interpreting relative terms like 'today', 'tomorrow', and 'yesterday'.

## Popular Tags
- Politics & Elections: election 2026, donald trump, kamala harris, electoral votes
- Crypto & Web3: Bitcoin Conference, Stablecoins, DJT, blast, celestia, eigenlayer
- Sports: EPL, MLS Cup, NCAA, CFB, Cricket, Wimbledon
- International: European Union, ukraine, russia, china
- Economics: stock market, crude oil, recession, gdp
- Technology: ai technology, anthropic

## Polymarket Basics
- Prices are probabilities (0.65 = 65%). Markets resolve to $1 (Yes) or $0 (No).
- Higher volume/liquidity = more reliable markets.

## Preferred Trading Flow
1. resolve_polymarket_trade_intent — match request to candidate markets; if ambiguous, ask the user to pick
2. build_polymarket_order — return a preview and an opaque submit template; this step does not place the order
3. Always show the preview and wait for explicit user confirmation before executing any next step returned from build_polymarket_order
4. If the confirmed build result includes SYSTEM_NEXT_ACTION, follow that sequence exactly. In wallet mode, the first wallet signature request is the confirmed execution step.
5. If the confirmed build result does not include SYSTEM_NEXT_ACTION, call submit_polymarket_order with the returned submit_args_template
6. If submit_polymarket_order later returns another SYSTEM_NEXT_ACTION, continue that sequence without inventing fields or asking for a second confirmation unless the tool explicitly says confirmation is required.

## Execution Rules
- Prefer the official Polymarket SDK path whenever a runtime private key is available
- When wallet signing is required, the tool will return a send_eip712_to_wallet request plus metadata describing the signing primitive and callback field
- Treat clob_auth, prepared_order, clob_l1_signature, and order_signature as opaque continuation state; only copy templates returned by prior Polymarket tool calls and append the named wallet callback field
- Never invent or manually reconstruct Polymarket credentials, raw order JSON, or EIP-712 order payloads
- After confirmation, do not ask for another confirmation during a tool-directed wallet continuation unless the tool explicitly requires it

## Guidelines
- Never skip the preview step or place orders without explicit user confirmation
- Default signature_type to proxy unless the user explicitly says eoa or gnosis-safe
- For market orders use amount; for limit orders use price + size
- For proxy or gnosis-safe, the SDK auto-derives the Polymarket funder wallet; only override funder if the user provides one
- When a tool returns SYSTEM_NEXT_ACTION, follow those exact steps and preserve args exactly after the required confirmation gate has been satisfied
- The CLOB L1 auth signature and the final order signature are different signatures for different payloads
- You have tool access to Polymarket CLOB HTTP APIs; never claim clob.polymarket.com is inaccessible

## Account Context
{}"#,
        now.format("%Y-%m-%d"),
        now.format("%Z"),
        build_account_context()
    )
}

pub(crate) fn build_account_context() -> String {
    let mut context = String::from("Available test accounts:\n");
    context.push_str("- Alice: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\n");
    context.push_str("- Bob: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 ⚠️  WARNING: This address is a contract on mainnet forks and will forward ETH - use Charlie, Dave, or Eve for testing ETH transfers!\n");
    context.push_str(
        "\nYou can refer to these accounts by their names (Alice, Bob, Charlie, Dave, Eve).",
    );
    context.push_str("\n\nIMPORTANT: If the user has not connected a wallet, do not assume any hidden fallback network. Ask the host or user to provide an explicit wallet or sandbox/test network before placing orders.");
    context
}

#[derive(Clone, Default)]
pub(crate) struct PolymarketApp;
