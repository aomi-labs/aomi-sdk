use crate::client::{
    decimal_to_prob, edge, limitless_public, limitless_signed, market_title, market_volume,
    no_price, odds_api, pct, urlencode, yes_price,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub(crate) struct GambitApp;

fn ok(value: Value) -> Result<Value, String> {
    Ok(json!({ "source": "gambit", "data": value }))
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[gambit] runtime: {e}"))
}

const MAX_BET: f64 = 10_000.0;
const MIN_BET: f64 = 1.0;

fn default_vec() -> Vec<Value> {
    vec![]
}

// ============================================================================
// Tool 1: gambit_find_football_markets
// ============================================================================

pub(crate) struct FindFootballMarkets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FindFootballMarketsArgs {
    /// Search query: "World Cup", "Argentina", "Premier League".
    pub query: String,
    #[serde(default)]
    pub limit: Option<i64>,
}

impl DynAomiTool for FindFootballMarkets {
    type App = GambitApp;
    type Args = FindFootballMarketsArgs;
    const NAME: &'static str = "gambit_find_football_markets";
    const DESCRIPTION: &'static str = "Search Limitless for football/soccer prediction markets. Returns simplified results with YES/NO prices, volume, and expiration. Use when user asks 'what matches can I bet on'. Public.";

    fn run(_app: &GambitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let limit = args.limit.unwrap_or(10);
            let path = format!("/markets/search?query={}&limit={}", urlencode(&args.query), limit);
            let resp = limitless_public(&path).await?;
            let arr = resp.as_array().map_or(&[] as &[_], |v| v);

            let markets: Vec<Value> = arr.iter().map(|m| {
                json!({
                    "title": market_title(m),
                    "slug": m.get("slug").and_then(|v| v.as_str()).unwrap_or(""),
                    "yes": pct(yes_price(m)),
                    "no": pct(no_price(m)),
                    "volume_usd": market_volume(m),
                    "expires": m.get("endDate").and_then(|v| v.as_str()).unwrap_or(""),
                    "has_liquidity": market_volume(m) > 100.0,
                })
            }).collect();

            ok(json!({
                "query": args.query,
                "count": markets.len(),
                "markets": markets,
            }))
        })
    }
}

// ============================================================================
// Tool 2: gambit_analyze_value_bet
// ============================================================================

pub(crate) struct AnalyzeValueBet;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AnalyzeValueBetArgs {
    pub slug: String,
    #[serde(default)]
    #[schemars(skip)]
    pub odds_api_key: Option<String>,
    #[serde(default)]
    pub home_team: Option<String>,
    #[serde(default)]
    pub away_team: Option<String>,
    #[serde(default)]
    pub sport: Option<String>,
}

impl DynAomiTool for AnalyzeValueBet {
    type App = GambitApp;
    type Args = AnalyzeValueBetArgs;
    const NAME: &'static str = "gambit_analyze_value_bet";
    const DESCRIPTION: &'static str = "Analyze a Limitless market against bookmaker odds. Calculates edge (mispricing) and recommends action. Core Gambit intelligence tool. Works with or without Odds API key.";

    fn run(_app: &GambitApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            // Fetch Limitless market
            let market_path = format!("/markets/{}", urlencode(&args.slug));
            let market = limitless_public(&market_path).await?;
            let yes = yes_price(&market);
            let no = no_price(&market);

            // Try bookmaker comparison
            let mut bookmaker = json!(null);
            if let Ok(odds_key) = resolve_secret_value(&ctx, args.odds_api_key.as_deref(), "ODDS_API_KEY", "") {
                let sport = args.sport.as_deref().unwrap_or("soccer_fifa_world_cup");
                let odds_path = format!("/sports/{}/odds/?regions=us,uk,eu&markets=h2h&oddsFormat=decimal", urlencode(sport));

                if let Ok(data) = odds_api(&odds_path, &odds_key).await {
                    let empty = default_vec();
                    let matches = data.as_array().unwrap_or(&empty);
                    for m in matches {
                        let home = m.get("home_team").and_then(|v| v.as_str()).unwrap_or("");
                        let away = m.get("away_team").and_then(|v| v.as_str()).unwrap_or("");

                        let home_match = args.home_team.as_deref().map(|h| home.to_lowercase().contains(&h.to_lowercase())).unwrap_or(false);
                        let away_match = args.away_team.as_deref().map(|a| away.to_lowercase().contains(&a.to_lowercase())).unwrap_or(false);

                        if home_match || away_match {
                            let mut best_home = 0.0_f64;
                            if let Some(bks) = m.get("bookmakers").and_then(|v| v.as_array()) {
                                for bk in bks {
                                    if let Some(markets) = bk.get("markets").and_then(|v| v.as_array()) {
                                        for bm in markets {
                                            if let Some(outcomes) = bm.get("outcomes").and_then(|v| v.as_array()) {
                                                for o in outcomes {
                                                    let name = o.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                                    let price = o.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                                    if name == home && price > best_home {
                                                        best_home = price;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let home_prob = decimal_to_prob(best_home);
                            let home_edge = edge(home_prob, yes);
                            let rec = if home_edge > 0.1 {
                                format!("STRONG VALUE — Limitless underprices YES by {:.0}%. Consider BUY YES.", home_edge * 100.0)
                            } else if home_edge > 0.05 {
                                format!("SLIGHT VALUE — {:.0}% edge on YES.", home_edge * 100.0)
                            } else if home_edge < -0.1 {
                                format!("OVERPRICED — Limitless overprices YES by {:.0}%. Consider BUY NO.", home_edge.abs() * 100.0)
                            } else {
                                "FAIR PRICE — No clear edge.".to_string()
                            };

                            bookmaker = json!({
                                "home": home, "away": away,
                                "bookmaker_decimal": format!("{:.2}", best_home),
                                "bookmaker_prob": pct(home_prob),
                                "limitless_yes": pct(yes),
                                "edge": format!("{:+.1}%", home_edge * 100.0),
                                "is_value": home_edge > 0.05,
                                "recommendation": rec,
                            });
                            break;
                        }
                    }
                }
            }

            let mut risks = Vec::new();
            if market_volume(&market) < 100.0 { risks.push("Thin volume.".to_string()); }
            if yes > 0.85 { risks.push("Heavy favorite — low return.".to_string()); }
            if yes < 0.15 { risks.push("Long shot — unlikely to hit.".to_string()); }

            ok(json!({
                "market": {
                    "title": market_title(&market),
                    "slug": args.slug,
                    "yes": pct(yes), "no": pct(no),
                    "volume_usd": market_volume(&market),
                },
                "bookmaker_comparison": bookmaker,
                "risks": risks,
            }))
        })
    }
}

// ============================================================================
// Tool 3: gambit_place_bet_simplified — preview, NOT execution
// ============================================================================

pub(crate) struct PlaceBetSimplified;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceBetSimplifiedArgs {
    pub amount_usd: f64,
    pub slug: String,
    pub outcome: String,
}

impl DynAomiTool for PlaceBetSimplified {
    type App = GambitApp;
    type Args = PlaceBetSimplifiedArgs;
    const NAME: &'static str = "gambit_place_bet_simplified";
    const DESCRIPTION: &'static str = "Build a bet preview: fetches market, checks liquidity, calculates cost/payout/risk. Returns a PREVIEW — does NOT execute. Safety: $1-$10,000 limits, thin liquidity warnings.";

    fn run(_app: &GambitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        // Safety checks
        if args.amount_usd < MIN_BET { return Err(format!("[gambit] min bet is ${MIN_BET}.")); }
        if args.amount_usd > MAX_BET { return Err(format!("[gambit] max bet is ${MAX_BET}.")); }
        let outcome = args.outcome.to_uppercase();
        if !matches!(outcome.as_str(), "YES" | "NO") { return Err(format!("[gambit] outcome must be YES or NO.")); }

        let runtime = rt()?;
        runtime.block_on(async move {
            let market = limitless_public(&format!("/markets/{}", urlencode(&args.slug))).await?;
            let yes = yes_price(&market);
            let no = no_price(&market);
            let price = if outcome == "YES" { yes } else { no };
            if price <= 0.0 || price >= 1.0 { return Err(format!("[gambit] invalid price: {price}")); }

            // Check orderbook
            let ob = limitless_public(&format!("/markets/{}/orderbook", urlencode(&args.slug))).await.unwrap_or(json!({}));
            let mut depth = 0.0_f64;
            if let Some(asks) = ob.get("asks").and_then(|v| v.as_array()) {
                for ask in asks {
                    let p = ask.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if p <= price * 1.05 { depth += ask.get("size").and_then(|v| v.as_f64()).unwrap_or(0.0); }
                }
            }

            let shares = args.amount_usd / price;
            let payout = shares;
            let profit = payout - args.amount_usd;
            let ret = (profit / args.amount_usd) * 100.0;
            let vol = market_volume(&market);

            let mut warnings = Vec::new();
            if args.amount_usd > 50.0 { warnings.push("Large bet.".to_string()); }
            if vol < 100.0 { warnings.push("Low volume.".to_string()); }
            if depth > 0.0 && shares > depth * 2.0 { warnings.push(format!("Thin liquidity ({:.0} shares available).", depth)); }
            if price > 0.85 { warnings.push("Heavy favorite.".to_string()); }
            if price < 0.15 { warnings.push("Long shot.".to_string()); }

            ok(json!({
                "stage": "preview",
                "message": "PREVIEW only. No order placed.",
                "market": { "title": market_title(&market), "slug": args.slug },
                "bet": { "outcome": outcome, "price": pct(price), "amount": format!("${:.2}", args.amount_usd), "shares": format!("{:.2}", shares) },
                "potential": { "payout": format!("${:.2}", payout), "profit": format!("${:.2}", profit), "return_pct": format!("{:.1}%", ret) },
                "liquidity": { "volume_usd": vol, "depth_shares": depth },
                "warnings": warnings,
            }))
        })
    }
}

// ============================================================================
// Tool 4: gambit_my_bets — plain English portfolio summary
// ============================================================================

pub(crate) struct GetMyBetsSummary;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMyBetsSummaryArgs {
    #[serde(default)]
    #[schemars(skip)]
    pub api_key: Option<String>,
    #[serde(default)]
    #[schemars(skip)]
    pub api_secret: Option<String>,
}

impl DynAomiTool for GetMyBetsSummary {
    type App = GambitApp;
    type Args = GetMyBetsSummaryArgs;
    const NAME: &'static str = "gambit_get_my_bets_summary";
    const DESCRIPTION: &'static str = "Plain-English summary of your open bets. Shows market, outcome, entry, current price, P&L. Requires Limitless API key + secret.";

    fn run(_app: &GambitApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let key = resolve_secret_value(&ctx, args.api_key.as_deref(), "LIMITLESS_API_KEY", "[gambit] missing LIMITLESS_API_KEY")?;
        let sec = resolve_secret_value(&ctx, args.api_secret.as_deref(), "LIMITLESS_API_SECRET", "[gambit] missing LIMITLESS_API_SECRET")?;

        let runtime = rt()?;
        runtime.block_on(async move {
            let data = limitless_signed(&key, &sec, "GET", "/portfolio/positions", "").await?;
            let empty = default_vec();
            let positions: Vec<Value> = data.as_array().unwrap_or(&empty).iter().map(|p| {
                let entry = p.get("avgPrice").or_else(|| p.get("entryPrice")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let current = p.get("currentPrice").and_then(|v| v.as_f64()).unwrap_or(entry);
                let size = p.get("size").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let pnl = (current - entry) * size;
                json!({
                    "market": p.get("market").or_else(|| p.get("marketTitle")).and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "outcome": p.get("outcome").and_then(|v| v.as_str()).unwrap_or("?"),
                    "shares": format!("{:.2}", size),
                    "entry": pct(entry), "current": pct(current),
                    "cost": format!("${:.2}", entry * size),
                    "value": format!("${:.2}", current * size),
                    "pnl": format!("{}${:.2}", if pnl >= 0.0 { "+" } else { "" }, pnl),
                })
            }).collect();

            let total_cost: f64 = positions.iter().filter_map(|p| p.get("cost")?.as_str()?.replace('$', "").parse::<f64>().ok()).sum();
            let total_val: f64 = positions.iter().filter_map(|p| p.get("value")?.as_str()?.replace('$', "").parse::<f64>().ok()).sum();
            let total_pnl = total_val - total_cost;

            ok(json!({
                "positions": positions,
                "count": positions.len(),
                "summary": format!(
                    "You have {} open bet{}. Invested: ${:.2}. Value: ${:.2}. P&L: {}${:.2}.",
                    positions.len(), if positions.len() == 1 { "" } else { "s" },
                    total_cost, total_val, if total_pnl >= 0.0 { "+" } else { "" }, total_pnl
                ),
            }))
        })
    }
}

// ============================================================================
// Tool 5: gambit_upcoming — matches + Limitless cross-reference
// ============================================================================

pub(crate) struct GetUpcomingBigMatches;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetUpcomingBigMatchesArgs {
    #[serde(default)]
    #[schemars(skip)]
    pub odds_api_key: Option<String>,
    #[serde(default)]
    pub sport: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

impl DynAomiTool for GetUpcomingBigMatches {
    type App = GambitApp;
    type Args = GetUpcomingBigMatchesArgs;
    const NAME: &'static str = "gambit_get_upcoming_big_matches";
    const DESCRIPTION: &'static str = "Upcoming football matches with Limitless prediction markets. Cross-references bookmaker odds with on-chain prices. Requires Odds API key.";

    fn run(_app: &GambitApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let odds_key = resolve_secret_value(&ctx, args.odds_api_key.as_deref(), "ODDS_API_KEY", "[gambit] missing ODDS_API_KEY")?;
        let sport = args.sport.unwrap_or_else(|| "soccer_fifa_world_cup".to_string());
        let region = args.region.unwrap_or_else(|| "us".to_string());

        let runtime = rt()?;
        runtime.block_on(async move {
            let data = odds_api(&format!("/sports/{}/odds/?regions={}&markets=h2h&oddsFormat=decimal", urlencode(&sport), urlencode(&region)), &odds_key).await?;
            let empty = default_vec();
            let matches = data.as_array().unwrap_or(&empty);
            let mut results = Vec::new();

            for m in matches {
                let home = m.get("home_team").and_then(|v| v.as_str()).unwrap_or("?");
                let away = m.get("away_team").and_then(|v| v.as_str()).unwrap_or("?");

                let mut best_home = 0.0_f64;
                let mut best_away = 0.0_f64;
                let mut bk_count = 0;
                if let Some(bks) = m.get("bookmakers").and_then(|v| v.as_array()) {
                    bk_count = bks.len();
                    for bk in bks {
                        if let Some(markets) = bk.get("markets").and_then(|v| v.as_array()) {
                            for bm in markets {
                                if let Some(outcomes) = bm.get("outcomes").and_then(|v| v.as_array()) {
                                    for o in outcomes {
                                        let name = o.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                        let price = o.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                        if name == home && price > best_home { best_home = price; }
                                        if name == away && price > best_away { best_away = price; }
                                    }
                                }
                            }
                        }
                    }
                }

                // Search Limitless
                let search = limitless_public(&format!("/markets/search?query={}&limit=3", urlencode(&format!("{} {}", home, away)))).await.unwrap_or(json!([]));
                let empty2 = default_vec();
                let lm_arr = search.as_array().unwrap_or(&empty2);
                let matched: Vec<Value> = lm_arr.iter().map(|lm| {
                    let lm_yes = yes_price(lm);
                    let hp = decimal_to_prob(best_home);
                    json!({
                        "title": market_title(lm),
                        "slug": lm.get("slug").and_then(|v| v.as_str()).unwrap_or(""),
                        "limitless_yes": pct(lm_yes),
                        "bookmaker_prob": pct(hp),
                        "edge": format!("{:+.1}%", edge(hp, lm_yes) * 100.0),
                        "is_value": edge(hp, lm_yes) > 0.05,
                    })
                }).collect();

                results.push(json!({
                    "match": format!("{} vs {}", home, away),
                    "kickoff": m.get("commence_time").and_then(|v| v.as_str()).unwrap_or(""),
                    "bookmakers": bk_count,
                    "best_odds": { "home": format!("{:.2}", best_home), "away": format!("{:.2}", best_away) },
                    "limitless_markets": matched,
                    "has_market": !matched.is_empty(),
                }));
            }

            let with_markets = results.iter().filter(|r| r["has_market"].as_bool().unwrap_or(false)).count();
            ok(json!({
                "sport": sport,
                "total_matches": results.len(),
                "with_markets": with_markets,
                "matches": results,
            }))
        })
    }
}

// ============================================================================
// Tool 6: gambit_market_pulse
// ============================================================================

pub(crate) struct MarketPulse;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MarketPulseArgs {
    pub category: String,
    #[serde(default)]
    pub limit: Option<i64>,
}

impl DynAomiTool for MarketPulse {
    type App = GambitApp;
    type Args = MarketPulseArgs;
    const NAME: &'static str = "gambit_market_pulse";
    const DESCRIPTION: &'static str = "Get the pulse of a market category — aggregate volume, sentiment, top markets. Use when user asks 'what's hot'. Public.";

    fn run(_app: &GambitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let limit = args.limit.unwrap_or(20);
            let query = match args.category.to_lowercase().as_str() {
                "football" | "soccer" => "football soccer world cup",
                "crypto" => "bitcoin ethereum crypto price",
                "politics" => "election president politics",
                _ => &args.category,
            };
            let resp = limitless_public(&format!("/markets/search?query={}&limit={}", urlencode(query), limit)).await.unwrap_or(json!([]));
            let empty = default_vec();
            let markets = resp.as_array().unwrap_or(&empty);

            let mut total_vol = 0.0_f64;
            let mut total_yes = 0.0_f64;
            let mut top: Vec<Value> = Vec::new();

            for m in markets {
                let y = yes_price(m);
                let v = market_volume(m);
                total_vol += v;
                total_yes += y;
                top.push(json!({ "title": market_title(m), "slug": m.get("slug").and_then(|v| v.as_str()).unwrap_or(""), "yes": pct(y), "volume": v }));
            }
            top.sort_by(|a, b| b.get("volume").and_then(|v| v.as_f64()).unwrap_or(0.0).partial_cmp(&a.get("volume").and_then(|v| v.as_f64()).unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));

            let count = markets.len();
            let avg = if count > 0 { total_yes / count as f64 } else { 0.0 };
            let sentiment = if avg > 0.6 { "BULLISH" } else if avg < 0.4 { "BEARISH" } else { "NEUTRAL" };

            ok(json!({
                "category": args.category,
                "markets": count,
                "total_volume": total_vol,
                "average_yes": pct(avg),
                "sentiment": sentiment,
                "top_markets": top.iter().take(5).cloned().collect::<Vec<_>>(),
            }))
        })
    }
}
