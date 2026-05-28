use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

// ── Tool 1: fanforge_launch_fan_coin ──────────────────────────────────────────

pub(crate) struct LaunchFanCoin;

impl DynAomiTool for LaunchFanCoin {
    type App = FanForgeApp;
    type Args = LaunchFanCoinArgs;
    const NAME: &'static str = "fanforge_launch_fan_coin";
    const DESCRIPTION: &'static str = "Launch a creator fan coin on Zora for a music creator. Takes the creator's name, ticker, and description — handles everything on-chain invisibly. Returns the live Zora link and coin address. Use when the creator says they want to launch a coin, start a fan economy, or monetize their fanbase.";

    fn run_with_routes(
        _app: &FanForgeApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let ticker = args.ticker.trim().to_uppercase();
        if ticker.len() < 3
            || ticker.len() > 5
            || !ticker.chars().all(|c| c.is_ascii_uppercase())
        {
            return Err(format!(
                "ticker_invalid: '{}' must be 3–5 letters (e.g. TEMI, VIBES)",
                ticker
            ));
        }

        ToolReturn::route(json!({
            "status": "setting_up",
            "message": "Checking your fan economy wallet…"
        }))
        .next(|next| {
            next.add::<host::GetAccountInfo>(json!({ "chain_id": 8453 }))
                .bind_as("creator_wallet");
        })
        .after_named(
            "fanforge_build_coin_tx",
            json!({
                "name": args.name,
                "ticker": ticker,
                "description": args.description,
                "image_url": args.image_url,
                "creator_telegram_id": args.creator_telegram_id,
            }),
        )
        .awaits("creator_wallet")
        .try_build()
    }
}

// ── Tool 1b (internal): fanforge_build_coin_tx ────────────────────────────────

pub(crate) struct BuildCoinTx;

impl DynAomiTool for BuildCoinTx {
    type App = FanForgeApp;
    type Args = BuildCoinTxArgs;
    const NAME: &'static str = "fanforge_build_coin_tx";
    const DESCRIPTION: &'static str = "Internal tool — called automatically after wallet lookup during coin launch. Uploads coin metadata to IPFS, fetches Zora transaction calldata, and stages the on-chain deployment. Do not call directly.";

    fn run_with_routes(
        _app: &FanForgeApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        // Extract the creator wallet address from the route-injected get_account_info result
        let creator_address = args
            .creator_wallet
            .as_str()
            .or_else(|| args.creator_wallet.get("address").and_then(Value::as_str))
            .or_else(|| {
                args.creator_wallet
                    .get("account")
                    .and_then(|a| a.get("address"))
                    .and_then(Value::as_str)
            })
            .ok_or("wallet_error: could not read your fan economy wallet address")?;

        // Build metadata JSON and upload to IPFS
        let metadata_json = json!({
            "name": args.name,
            "description": args.description,
            "symbol": args.ticker,
            "image": args.image_url.as_deref().unwrap_or(""),
        });
        let metadata_uri = ipfs_pin_json(&metadata_json)?;

        // Fetch transaction calldata from Zora's REST API
        let zora_req = json!({
            "creator": creator_address,
            "name": args.name,
            "symbol": args.ticker,
            "metadata": { "type": "RAW_URI", "uri": metadata_uri },
            "currency": "CREATOR_COIN",
            "chainId": 8453,
        });
        let calldata_resp = zora_post("/create/content", &zora_req)?;

        let call = calldata_resp
            .get("calls")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .ok_or("zora_error: Zora API returned no transaction calldata")?;

        let to = call
            .get("to")
            .and_then(Value::as_str)
            .ok_or("zora_error: missing transaction target address")?;
        let data = call
            .get("data")
            .and_then(Value::as_str)
            .ok_or("zora_error: missing transaction calldata")?;
        let value_str = call.get("value").and_then(Value::as_str).unwrap_or("0");
        let predicted_address = calldata_resp
            .get("predictedCoinAddress")
            .and_then(Value::as_str)
            .unwrap_or("");

        ToolReturn::route(json!({
            "status": "deploying",
            "message": "Deploying your fan coin — please approve in your wallet.",
        }))
        .next(|next| {
            next.add::<host::StageTx>(json!({
                "to": to,
                "data": { "raw": data },
                "value": value_str,
                "chain_id": 8453,
            }))
            .enforce(EnforcementPolicy::Stop, |enforce| {
                enforce.add::<host::SimulateBatch>(json!({}));
                enforce
                    .add::<host::CommitTxs>(json!({ "aa_preference": "auto" }))
                    .bind_as("transaction_hash");
            });
        })
        .after_named(
            "fanforge_finalize_launch",
            json!({
                "name": args.name,
                "ticker": args.ticker,
                "creator_telegram_id": args.creator_telegram_id,
                "predicted_coin_address": predicted_address,
            }),
        )
        .awaits("transaction_hash")
        .try_build()
    }
}

// ── Tool 1c (internal): fanforge_finalize_launch ──────────────────────────────

pub(crate) struct FinalizeLaunch;

impl DynAomiTool for FinalizeLaunch {
    type App = FanForgeApp;
    type Args = FinalizeLaunchArgs;
    const NAME: &'static str = "fanforge_finalize_launch";
    const DESCRIPTION: &'static str = "Internal tool — called automatically after the fan coin transaction is confirmed. Saves the coin record and returns the live Zora link. Do not call directly.";

    fn run(
        _app: &FanForgeApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        // Unwrap the tx hash from whatever commit_txs returns
        let tx_hash = args
            .transaction_hash
            .as_str()
            .or_else(|| {
                args.transaction_hash
                    .get("hash")
                    .and_then(Value::as_str)
            })
            .or_else(|| {
                args.transaction_hash
                    .get("transactionHash")
                    .and_then(Value::as_str)
            })
            .unwrap_or("pending");

        let zora_url = format!(
            "https://zora.co/coin/base:{}",
            args.predicted_coin_address.to_lowercase()
        );

        // Best-effort: store coin in Supabase (don't fail the launch if DB is unavailable)
        let _ = supabase_post(
            "creator_coins",
            &json!({
                "creator_telegram_id": args.creator_telegram_id,
                "coin_address": args.predicted_coin_address,
                "ticker": args.ticker,
                "name": args.name,
                "transaction_hash": tx_hash,
                "zora_url": zora_url,
            }),
        );

        ok(json!({
            "status": "launched",
            "coin_name": args.name,
            "ticker": args.ticker,
            "zora_url": zora_url,
            "coin_address": args.predicted_coin_address,
            "message": format!(
                "Your {} fan coin (${}) is live! Share this link with your fans: {}",
                args.name, args.ticker, zora_url
            ),
        }))
    }
}

// ── Tool 2: fanforge_get_fan_leaderboard ──────────────────────────────────────

pub(crate) struct GetFanLeaderboard;

impl DynAomiTool for GetFanLeaderboard {
    type App = FanForgeApp;
    type Args = GetFanLeaderboardArgs;
    const NAME: &'static str = "fanforge_get_fan_leaderboard";
    const DESCRIPTION: &'static str = "Get the ranked leaderboard of top fans holding a creator's Zora coin. Returns wallet addresses (truncated), balances, and percentage of total supply held. Use when the creator asks who their top fans are, wants to see holder rankings, or is deciding who gets a reward.";

    fn run(_app: &FanForgeApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let limit = args.limit.unwrap_or(10).min(50);
        let path = format!(
            "/coinHolders?address={}&chainId=8453&count={}",
            urlencode(&args.coin_address),
            limit
        );

        let resp = zora_get(&path)?;

        // Response shape: { zora20Token: { tokenBalances: { edges: [{ node: { balance, ownerAddress, ownerProfile } }] } } }
        let edges = resp
            .get("zora20Token")
            .and_then(|t| t.get("tokenBalances"))
            .and_then(|tb| tb.get("edges"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let entries: Vec<Value> = edges
            .into_iter()
            .enumerate()
            .map(|(i, edge)| {
                let node = edge.get("node").unwrap_or(&edge);
                let wallet = node
                    .get("ownerAddress")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let wallet_short = if wallet.len() > 10 {
                    format!("{}…{}", &wallet[..6], &wallet[wallet.len() - 4..])
                } else {
                    wallet.to_string()
                };
                let handle = node
                    .get("ownerProfile")
                    .and_then(|p| p.get("handle"))
                    .and_then(Value::as_str)
                    .unwrap_or(&wallet_short)
                    .to_string();
                // balance is raw (18 decimals) — convert to human-readable coins
                let balance_coins = node
                    .get("balance")
                    .and_then(Value::as_str)
                    .unwrap_or("0")
                    .parse::<f64>()
                    .map(|b| b / 1e18)
                    .unwrap_or(0.0);

                json!({
                    "rank": i + 1,
                    "wallet_short": wallet_short,
                    "handle": handle,
                    "balance": format!("{:.2}", balance_coins),
                })
            })
            .collect();

        ok(json!({
            "coin_address": args.coin_address,
            "total_entries": entries.len(),
            "leaderboard": entries,
        }))
    }
}

// ── Tool 3: fanforge_create_fan_mission ───────────────────────────────────────

pub(crate) struct CreateFanMission;

impl DynAomiTool for CreateFanMission {
    type App = FanForgeApp;
    type Args = CreateFanMissionArgs;
    const NAME: &'static str = "fanforge_create_fan_mission";
    const DESCRIPTION: &'static str = "Create a fan mission that unlocks exclusive content for holders meeting a minimum coin balance. Stores the mission and makes it ready for distribution. Use when the creator wants to reward fans who hold a certain amount of their coin with exclusive content (unreleased tracks, early access, etc.).";

    fn run(_app: &FanForgeApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        if args.threshold <= 0.0 {
            return Err("threshold_invalid: minimum coin balance must be greater than 0".to_string());
        }

        let expires_at = args.expires_at.unwrap_or_else(|| {
            // Default 30 days from now
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let thirty_days = now + 30 * 24 * 3600;
            format!("{}", thirty_days)
        });

        let mission_row = json!({
            "coin_address": args.coin_address,
            "title": args.title,
            "content_url": args.content_url,
            "threshold": args.threshold,
            "expires_at": expires_at,
            "status": "active",
        });

        let result = supabase_post("fan_missions", &mission_row)?;

        let mission_id = result
            .as_array()
            .and_then(|a| a.first())
            .and_then(|r| r.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();

        ok(json!({
            "mission_id": mission_id,
            "title": args.title,
            "threshold": args.threshold,
            "status": "active",
            "message": "Mission created. Use fanforge_distribute_rewards to deliver content to eligible fans.",
        }))
    }
}

// ── Tool 4: fanforge_distribute_rewards ───────────────────────────────────────

pub(crate) struct DistributeRewards;

impl DynAomiTool for DistributeRewards {
    type App = FanForgeApp;
    type Args = DistributeRewardsArgs;
    const NAME: &'static str = "fanforge_distribute_rewards";
    const DESCRIPTION: &'static str = "Distribute exclusive content to all fans who meet the mission's holding threshold. Checks current coin holders, filters by the minimum balance, and delivers the content URL to every eligible fan who hasn't received it yet. Use after creating a mission or when the creator asks to send rewards to fans.";

    fn run(_app: &FanForgeApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        // 1. Load the mission from Supabase
        let missions = supabase_get(
            "fan_missions",
            &format!("id=eq.{}&select=*", urlencode(&args.mission_id)),
        )?;

        let mission = missions
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .ok_or_else(|| format!("mission_not_found: no mission with id {}", args.mission_id))?;

        let status = mission.get("status").and_then(Value::as_str).unwrap_or("");
        if status != "active" {
            return Err(format!("mission_not_active: mission status is '{status}'"));
        }

        let coin_address = mission
            .get("coin_address")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let threshold: f64 = mission
            .get("threshold")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);

        // 2. Get current leaderboard (all holders)
        let path = format!(
            "/coinHolders?address={}&chainId=8453&count=100",
            urlencode(&coin_address)
        );
        let holders_resp = zora_get(&path)?;
        // Response shape: { zora20Token: { tokenBalances: { edges: [{ node: { balance, ownerAddress } }] } } }
        let edges = holders_resp
            .get("zora20Token")
            .and_then(|t| t.get("tokenBalances"))
            .and_then(|tb| tb.get("edges"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        // 3. Load already-distributed wallets for this mission
        let distributed = supabase_get(
            "reward_distributions",
            &format!("mission_id=eq.{}&select=recipient_wallet", urlencode(&args.mission_id)),
        )?;
        let already_sent: std::collections::HashSet<String> = distributed
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|r| r.get("recipient_wallet").and_then(Value::as_str))
            .map(String::from)
            .collect();

        // 4. Filter eligible wallets (balance >= threshold, not yet delivered)
        let mut newly_dispatched = 0u32;
        let mut eligible_count = 0u32;

        for edge in &edges {
            let node = edge.get("node").unwrap_or(edge);
            let wallet = node
                .get("ownerAddress")
                .and_then(Value::as_str)
                .unwrap_or("");
            // balance is raw (18 decimals) — convert to coins for threshold comparison
            let balance: f64 = node
                .get("balance")
                .and_then(Value::as_str)
                .unwrap_or("0")
                .parse::<f64>()
                .map(|b| b / 1e18)
                .unwrap_or(0.0);

            if balance < threshold {
                continue;
            }
            eligible_count += 1;

            if already_sent.contains(wallet) {
                continue;
            }

            let dist_row = json!({
                "mission_id": args.mission_id,
                "recipient_wallet": wallet,
            });
            if supabase_post("reward_distributions", &dist_row).is_ok() {
                newly_dispatched += 1;
            }
        }

        ok(json!({
            "mission_id": args.mission_id,
            "eligible_count": eligible_count,
            "newly_dispatched": newly_dispatched,
            "already_received": eligible_count.saturating_sub(newly_dispatched),
            "status": "complete",
            "message": format!(
                "{newly_dispatched} fans just unlocked the content. {} had already received it.",
                eligible_count.saturating_sub(newly_dispatched)
            ),
        }))
    }
}

// ── Tool 5: fanforge_get_creator_recap ────────────────────────────────────────

pub(crate) struct GetCreatorRecap;

impl DynAomiTool for GetCreatorRecap {
    type App = FanForgeApp;
    type Args = GetCreatorRecapArgs;
    const NAME: &'static str = "fanforge_get_creator_recap";
    const DESCRIPTION: &'static str = "Generate a plain-English weekly recap for the creator showing how their fan coin and community are growing. Returns a summary and a ready-to-copy Twitter post. Use when the creator asks how their coin is doing, wants a weekly update, or asks for something to post on social media.";

    fn run(_app: &FanForgeApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let days = args.days.unwrap_or(7);

        // Fetch current coin data
        let coin_path = format!(
            "/coin?address={}&chain=8453",
            urlencode(&args.coin_address)
        );
        let coin_data = zora_get(&coin_path)?;

        // Response shape: { zora20Token: { name, symbol, uniqueHolders, marketCap, volume24h, ... } }
        let token = coin_data
            .get("zora20Token")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let coin_name = token
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("your coin");
        let ticker = token
            .get("symbol")
            .and_then(Value::as_str)
            .unwrap_or("");
        let holders = token
            .get("uniqueHolders")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let market_cap = token
            .get("marketCap")
            .and_then(Value::as_str)
            .unwrap_or("0");
        let volume_24h = token
            .get("volume24h")
            .and_then(Value::as_str)
            .unwrap_or("0");

        // Fetch missions activity from Supabase
        let missions = supabase_get(
            "fan_missions",
            &format!("coin_address=eq.{}&status=eq.active&select=id,title", urlencode(&args.coin_address)),
        ).unwrap_or(json!([]));
        let active_missions = missions.as_array().map(|a| a.len()).unwrap_or(0);

        let summary = format!(
            "Over the last {days} days, your fan economy is alive. \
             ${ticker} has {holders} fans holding your coin. \
             You have {active_missions} active mission{} unlocking exclusive content for your top supporters. \
             Market cap: ${market_cap}. 24h trading volume: ${volume_24h}.",
            if active_missions == 1 { "" } else { "s" }
        );

        let twitter_post = format!(
            "my fan coin ${ticker} now has {holders} holders 🎵\n\
             if you're holding, you already know what's inside 🔐\n\
             new mission drops soon for my top fans\n\
             link in bio to join the movement",
        );

        ok(json!({
            "coin_name": coin_name,
            "ticker": ticker,
            "total_holders": holders,
            "active_missions": active_missions,
            "days_covered": days,
            "summary": summary,
            "twitter_post": twitter_post,
        }))
    }
}
