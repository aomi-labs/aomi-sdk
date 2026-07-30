//! World Markets domain: a small in-crate market catalog plus trade math.
//!
//! Self-contained by design (see `lib.rs`) — the app demonstrates the
//! app-scoped skill construct, so the catalog is static data rather than a
//! live venue API. `ROUTER` and `BUY_SELECTOR` mirror the values pinned in
//! `skill/guard.json`; the guard table is what actually enforces them at
//! stage time.

use serde::Serialize;

/// Trade router the built calldata targets. Must stay in lockstep with
/// `contracts.ROUTER` in `skill/guard.json` — the staged transaction is
/// vetted against the guard table, not against this constant.
pub const ROUTER: &str = "0x7a2088a1bfc9d81c55368ae168c2c02570cb814f";

/// `buyOutcome(uint256 marketId, uint256 outcome, uint256 usdcAmount)`.
/// Mirrors `selectors.BUY_OUTCOME` in `skill/guard.json`.
pub const BUY_SELECTOR: &str = "0xa9059c5b";

/// USDC has 6 decimals; notional caps in the guard table are whole USD.
pub const USDC_DECIMALS: u32 = 6;

#[derive(Clone, Default)]
pub struct WorldMarketsApp;

#[derive(Debug, Clone, Serialize)]
pub struct Market {
    pub id: u64,
    pub question: &'static str,
    /// Implied probability of YES in basis points.
    pub yes_bps: u64,
    pub chain_id: u64,
}

pub const MARKETS: [Market; 3] = [
    Market {
        id: 1,
        question: "Will global average temperature set a new record in 2026?",
        yes_bps: 6_400,
        chain_id: 1,
    },
    Market {
        id: 2,
        question: "Will the ECB cut rates before Q4 2026?",
        yes_bps: 4_150,
        chain_id: 1,
    },
    Market {
        id: 3,
        question: "Will BTC close 2026 above $150k?",
        yes_bps: 3_300,
        chain_id: 1,
    },
];

pub fn market(id: u64) -> Option<&'static Market> {
    MARKETS.iter().find(|market| market.id == id)
}

/// Price of one outcome share in USD micro-units (USDC base units).
pub fn share_price_usdc(market: &Market, yes: bool) -> u64 {
    let bps = if yes { market.yes_bps } else { 10_000 - market.yes_bps };
    // 1 share pays out 1 USDC; price = probability.
    bps * 10u64.pow(USDC_DECIMALS) / 10_000
}

/// ABI-encode `buyOutcome(marketId, outcome, usdcAmount)` calldata.
pub fn buy_calldata(market_id: u64, yes: bool, usdc_amount: u64) -> String {
    let mut data = String::from(BUY_SELECTOR);
    for word in [market_id, u64::from(yes), usdc_amount] {
        data.push_str(&format!("{word:064x}"));
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calldata_is_selector_plus_three_words() {
        let data = buy_calldata(2, true, 25_000_000);
        assert!(data.starts_with(BUY_SELECTOR));
        assert_eq!(data.len(), BUY_SELECTOR.len() + 3 * 64);
        assert!(data.ends_with(&format!("{:064x}", 25_000_000u64)));
    }

    #[test]
    fn share_price_splits_probability() {
        let market = market(1).unwrap();
        assert_eq!(share_price_usdc(market, true), 640_000);
        assert_eq!(share_price_usdc(market, false), 360_000);
    }
}
