//! Trustless Work single-release escrow creation on Arc Testnet.
//!
//! `trustless_work_review_escrow` checks the agreement locally, asks the
//! Trustless Work Arc API to build the unsigned deploy, decodes that build and
//! refuses it unless every field matches the agreement and the escrow address
//! derives from the same calldata. The verified build is kept for the session
//! under its address. `trustless_work_simulate_escrow` stages and simulates
//! only that build, and only by the address the user reviewed.

use alloy_primitives::{Address, U256, address, keccak256};
use alloy_sol_types::{SolCall, SolValue, sol};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const API_URL: &str = "https://arc-api-demo-production.up.railway.app";
const ARC_TESTNET_CHAIN_ID: u64 = 5_042_002;
// `GET /health` on the Trustless Work Arc API.
const FACTORY: Address = address!("0x829fE7040797Df140636901e79F1bA448B7e9ae0");
// `factory.implementation()` on Arc Testnet; every escrow is an EIP-1167 clone of it.
const IMPLEMENTATION: Address = address!("0x1F0E0C5b704B3e10609Bf4862071Db815a0c839b");
const USDC: Address = address!("0x3600000000000000000000000000000000000000");
// `TRUSTLESS_WORK_FEE_BPS` in SingleReleaseEscrow.sol.
const TRUSTLESS_WORK_FEE_BPS: u128 = 30;
const BUILD_TTL: Duration = Duration::from_secs(15 * 60);

sol! {
    #[derive(Debug, PartialEq, Eq)]
    struct MilestoneApprovals { uint32 target; uint32 approvalCount; address[] approvedBy; }
    #[derive(Debug, PartialEq, Eq)]
    struct Milestone { string description; string status; string evidence; MilestoneApprovals approvals; }
    #[derive(Debug, PartialEq, Eq)]
    struct Roles {
        address[] approvers;
        address[] serviceProviders;
        address platform;
        address[] releaseSigners;
        address[] disputeResolvers;
        address receiver;
        address admin;
        address[] observers;
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Dispute { bool isDisputed; string reason; bool resolved; }
    #[derive(Debug, PartialEq, Eq)]
    struct Escrow {
        string engagementId;
        string title;
        Roles roles;
        string description;
        uint256 amount;
        uint32 platformFee;
        Milestone[] milestones;
        Dispute dispute;
        bool released;
        address token;
        uint32 receiverMemo;
    }
    function createEscrow(bytes32 salt, Escrow escrowProperties) returns (address escrow);
}

/// A build the user has been shown, keyed by session. Only the latest build
/// of a session can be simulated, and only before it expires.
struct ReviewedBuild {
    contract_id: Address,
    signer: Address,
    data: String,
    created: Instant,
}

static BUILDS: Mutex<BTreeMap<String, ReviewedBuild>> = Mutex::new(BTreeMap::new());

#[derive(Clone, Default)]
pub(crate) struct TrustlessWorkApp;

enum Decimal {
    Exact(String, u128),
    TooPrecise,
    Malformed,
}

/// Parse a human decimal ("10", "10.5") into its normalized text and its value
/// scaled by `places` decimals. Trailing zeros do not count as precision.
fn decimal(value: &Value, places: usize) -> Decimal {
    let text = match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return Decimal::Malformed,
    };
    let (whole, frac) = text.split_once('.').unwrap_or((&text, ""));
    if whole.is_empty()
        || (text.contains('.') && frac.is_empty())
        || !whole
            .bytes()
            .chain(frac.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return Decimal::Malformed;
    }
    let whole = match whole.trim_start_matches('0') {
        "" => "0",
        w => w,
    };
    let frac = frac.trim_end_matches('0');
    if frac.len() > places {
        return Decimal::TooPrecise;
    }
    // Nine whole digits keep the value exact as the JSON number the API takes.
    if whole.len() > 9 {
        return Decimal::Malformed;
    }
    let scaled = format!("{whole}{frac:0<places$}").parse().unwrap_or(0);
    let text = if frac.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{frac}")
    };
    Decimal::Exact(text, scaled)
}

/// USDC base units (6 decimals) as a human amount.
fn usdc(units: u128) -> String {
    let frac = format!("{:06}", units % 1_000_000);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{}", units / 1_000_000)
    } else {
        format!("{}.{frac}", units / 1_000_000)
    }
}

/// The connected wallet, which must be on Arc Testnet. It signs the deploy.
fn arc_wallet(ctx: &DynToolCallCtx) -> Result<Address, String> {
    let chain_id = ctx.attribute_u64(&["domain", "evm", "chain_id"]);
    if chain_id != Some(ARC_TESTNET_CHAIN_ID) {
        return Err(format!(
            "Switch the wallet to Arc Testnet ({ARC_TESTNET_CHAIN_ID}) first; Trustless Work escrows here are testnet only."
        ));
    }
    ctx.attribute_string(&["domain", "evm", "address"])
        .and_then(|a| a.parse::<Address>().ok())
        .filter(|a| !a.is_zero())
        .ok_or_else(|| "Connect the wallet that will create the escrow.".to_string())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RolesArgs {
    /// Wallets that approve milestones (1-5).
    pub approvers: Vec<String>,
    /// Wallets that do the work and update milestone status (1-5).
    pub service_providers: Vec<String>,
    /// Platform wallet. Receives the platform fee.
    pub platform: String,
    /// Wallets allowed to release the funds (1-5).
    pub release_signers: Vec<String>,
    /// Wallets that resolve disputes (1-5). Cannot hold any other role.
    pub dispute_resolvers: Vec<String>,
    /// Final beneficiary of the released funds.
    pub receiver: String,
    /// Wallet that can update the escrow and its milestones.
    pub admin: String,
    /// Read-only wallets with no on-chain authority (0-5).
    #[serde(default)]
    pub observers: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MilestoneArgs {
    /// What must be delivered for this milestone.
    pub description: String,
    /// Distinct approvals required, at most the number of approvers. Defaults to 1.
    #[serde(default)]
    pub approvals_target: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviewEscrowArgs {
    /// Agreement or engagement reference (max 100 characters).
    pub engagement_id: String,
    /// Escrow title (max 100 characters).
    pub title: String,
    /// What the escrow pays for (max 500 characters).
    pub description: String,
    /// Wallet address for each role.
    pub roles: RolesArgs,
    /// Total escrow amount in normal USDC units as a decimal string: "10" means 10 USDC. Never base units; at most 6 decimal places.
    pub amount: Value,
    /// Platform fee as a percentage: "2" means 2%. At most 2 decimal places.
    pub platform_fee: Value,
    /// Milestones in the agreement (1-50).
    pub milestones: Vec<MilestoneArgs>,
}

pub(crate) struct ReviewEscrow;

impl DynAomiTool for ReviewEscrow {
    type App = TrustlessWorkApp;
    type Args = ReviewEscrowArgs;
    const NAME: &'static str = "trustless_work_review_escrow";
    const DESCRIPTION: &'static str = "Build a Trustless Work single-release escrow for a service agreement on Arc Testnet and return its details for the user to review: escrow address (contract_id), amount, fees, roles and milestones. The Trustless Work API builds the unsigned deploy; this tool verifies it matches the agreement. Nothing is staged or signed. Errors such as AMOUNT_PRECISION_UNSUPPORTED or ESCROW_DISPUTE_RESOLVER_OVERLAPS_WITH_OTHER_ROLE stop the flow. Each call creates a new contract_id, so the user must review the latest one.";

    fn run(
        _app: &TrustlessWorkApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let signer = arc_wallet(&ctx)?;

        let addr = |role: &str, s: &str| {
            s.trim()
                .parse::<Address>()
                .map_err(|_| format!("The {role} wallet is not a valid address: {s}"))
        };
        let addrs = |role: &str, list: &[String]| {
            list.iter()
                .map(|s| addr(role, s))
                .collect::<Result<Vec<_>, _>>()
        };
        let roles = Roles {
            approvers: addrs("approver", &args.roles.approvers)?,
            serviceProviders: addrs("service provider", &args.roles.service_providers)?,
            platform: addr("platform", &args.roles.platform)?,
            releaseSigners: addrs("release signer", &args.roles.release_signers)?,
            disputeResolvers: addrs("dispute resolver", &args.roles.dispute_resolvers)?,
            receiver: addr("receiver", &args.roles.receiver)?,
            admin: addr("admin", &args.roles.admin)?,
            observers: addrs("observer", &args.roles.observers)?,
        };

        // Same check, order and code as the contract and API (rule 4).
        for resolver in &roles.disputeResolvers {
            let overlap = if roles.approvers.contains(resolver) {
                Some("an approver")
            } else if roles.serviceProviders.contains(resolver) {
                Some("a service provider")
            } else if roles.releaseSigners.contains(resolver) {
                Some("a release signer")
            } else if *resolver == roles.receiver {
                Some("the receiver")
            } else if *resolver == roles.platform {
                Some("the platform")
            } else {
                None
            };
            if let Some(role) = overlap {
                return Err(format!(
                    "ESCROW_DISPUTE_RESOLVER_OVERLAPS_WITH_OTHER_ROLE (HTTP 422): dispute resolver {resolver} is also {role}. A dispute resolver cannot hold any other role. Nothing was built; choose a different resolver and review again."
                ));
            }
        }

        // Normal USDC units, at most 6 decimals (rules 1 and 2).
        let (amount, amount_units) = match decimal(&args.amount, 6) {
            Decimal::Exact(text, units) if units > 0 => (text, units),
            Decimal::TooPrecise => {
                return Err(format!(
                    "AMOUNT_PRECISION_UNSUPPORTED: {} has more than 6 decimal places, the most USDC supports. Nothing was built; round the agreement amount and review again.",
                    args.amount
                ));
            }
            _ => {
                return Err(format!(
                    "The amount {} is not a positive USDC amount. Give it in normal units as a decimal string, like \"10\" for 10 USDC.",
                    args.amount
                ));
            }
        };
        // Percent, at most 2 decimals, so basis points are exact (rule 3).
        let (fee, fee_bps) = match decimal(&args.platform_fee, 2) {
            Decimal::Exact(text, bps) if bps + TRUSTLESS_WORK_FEE_BPS <= 10_000 => (text, bps),
            _ => {
                return Err(format!(
                    "The platform fee {} must be a percentage with at most 2 decimals, like \"2\" for 2%, and below 99.7%.",
                    args.platform_fee
                ));
            }
        };
        if args.milestones.is_empty() || args.milestones.len() > 50 {
            return Err("The agreement needs between 1 and 50 milestones.".to_string());
        }

        let expected = Escrow {
            engagementId: args.engagement_id.clone(),
            title: args.title.clone(),
            roles,
            description: args.description.clone(),
            amount: U256::from(amount_units),
            platformFee: fee_bps as u32,
            milestones: args
                .milestones
                .iter()
                .map(|m| Milestone {
                    description: m.description.clone(),
                    status: "pending".to_string(),
                    evidence: String::new(),
                    approvals: MilestoneApprovals {
                        target: m.approvals_target.unwrap_or(1),
                        approvalCount: 0,
                        approvedBy: vec![],
                    },
                })
                .collect(),
            dispute: Dispute {
                isDisputed: false,
                reason: String::new(),
                resolved: false,
            },
            released: false,
            token: USDC,
            receiverMemo: 0,
        };

        let api_key = resolve_secret_value(
            &ctx,
            None,
            "TRUSTLESS_WORK_API_KEY",
            "Add the Trustless Work API key (TRUSTLESS_WORK_API_KEY) in this app's settings.",
        )?;
        let r = &expected.roles;
        let body = json!({
            "signer": signer.to_string(),
            "engagementId": expected.engagementId,
            "title": expected.title,
            "description": expected.description,
            "roles": {
                "approvers": r.approvers.iter().map(Address::to_string).collect::<Vec<_>>(),
                "serviceProviders": r.serviceProviders.iter().map(Address::to_string).collect::<Vec<_>>(),
                "platform": r.platform.to_string(),
                "releaseSigners": r.releaseSigners.iter().map(Address::to_string).collect::<Vec<_>>(),
                "disputeResolvers": r.disputeResolvers.iter().map(Address::to_string).collect::<Vec<_>>(),
                "receiver": r.receiver.to_string(),
                "admin": r.admin.to_string(),
                "observers": r.observers.iter().map(Address::to_string).collect::<Vec<_>>(),
            },
            // Human units as JSON numbers: 10 means 10 USDC, 2 means 2%.
            "amount": serde_json::from_str::<Value>(&amount).map_err(|e| e.to_string())?,
            "platformFee": serde_json::from_str::<Value>(&fee).map_err(|e| e.to_string())?,
            "milestones": expected.milestones.iter().map(|m| json!({
                "description": m.description,
                "status": m.status,
                "approvalsTarget": m.approvals.target,
            })).collect::<Vec<_>>(),
            "token": USDC.to_string(),
            "receiverMemo": 0,
        });

        let response = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("[trustless-work] HTTP client: {e}"))?
            .post(format!("{API_URL}/escrow/single-release/v2/deploy"))
            .header("x-api-key", api_key)
            .json(&body)
            .send()
            .map_err(
                |_| "Trustless Work API is unreachable. Nothing was built; try again later.",
            )?;
        let status = response.status();
        let payload: Value = response.json().unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(format!(
                "{} (HTTP {}): {} Trustless Work refused to build the escrow, so nothing was built. Fix the agreement and review again.",
                payload["code"].as_str().unwrap_or("TRUSTLESS_WORK_ERROR"),
                status.as_u16(),
                payload["detail"].as_str().unwrap_or("no detail returned."),
            ));
        }

        let (review, build) = check_build(&expected, signer, &fee, &payload)?;
        let mut builds = BUILDS.lock().map_err(|_| "build store poisoned")?;
        builds.retain(|_, b| b.created.elapsed() < BUILD_TTL);
        builds.insert(ctx.session_id.clone(), build);
        Ok(review)
    }
}

/// Decode the API's build and refuse it unless it deploys exactly `expected`
/// from `signer`, and its `contractId` is the address that calldata creates.
fn check_build(
    expected: &Escrow,
    signer: Address,
    fee: &str,
    payload: &Value,
) -> Result<(Value, ReviewedBuild), String> {
    let tx = &payload["transaction"];
    let hex_u64 = |field: &str| {
        tx[field]
            .as_str()
            .and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok())
    };
    let data = tx["data"].as_str().unwrap_or_default();
    let call = alloy_primitives::hex::decode(data)
        .ok()
        .and_then(|bytes| createEscrowCall::abi_decode(&bytes).ok());
    let contract_id = payload["contractId"]
        .as_str()
        .and_then(|a| a.parse::<Address>().ok());
    let from = tx["from"].as_str().and_then(|a| a.parse::<Address>().ok());
    let to = tx["to"].as_str().and_then(|a| a.parse::<Address>().ok());
    let (Some(call), Some(contract_id)) = (call, contract_id) else {
        return Err(
            "Trustless Work returned a build this app cannot read. Nothing was staged; stop."
                .into(),
        );
    };
    let escrow = &call.escrowProperties;

    let mismatch = [
        ("chainId", hex_u64("chainId") == Some(ARC_TESTNET_CHAIN_ID)),
        ("from", from == Some(signer)),
        ("to", to == Some(FACTORY)),
        ("value", hex_u64("value") == Some(0)),
        ("engagementId", escrow.engagementId == expected.engagementId),
        ("title", escrow.title == expected.title),
        ("description", escrow.description == expected.description),
        ("roles", escrow.roles == expected.roles),
        ("amount", escrow.amount == expected.amount),
        ("platformFee", escrow.platformFee == expected.platformFee),
        ("milestones", escrow.milestones == expected.milestones),
        ("dispute", escrow.dispute == expected.dispute),
        ("released", escrow.released == expected.released),
        ("token", escrow.token == expected.token),
        ("receiverMemo", escrow.receiverMemo == expected.receiverMemo),
    ]
    .into_iter()
    .find(|(_, same)| !same);
    if let Some((field, _)) = mismatch {
        return Err(format!(
            "The transaction Trustless Work built does not match the agreement ({field}). Nothing was staged; stop and review again."
        ));
    }

    // Rule 5: the address shown must be the one this exact calldata creates.
    // Factory: cloneDeterministic(implementation, keccak256(abi.encode(creator, salt))).
    let mut init_code = alloy_primitives::hex!("3d602d80600a3d3981f3363d3d373d3d3d363d73").to_vec();
    init_code.extend_from_slice(IMPLEMENTATION.as_slice());
    init_code.extend_from_slice(&alloy_primitives::hex!("5af43d82803e903d91602b57fd5bf3"));
    let derived = FACTORY.create2(
        keccak256((signer, call.salt).abi_encode()),
        keccak256(&init_code),
    );
    if derived != contract_id {
        return Err(format!(
            "Trustless Work reported escrow address {contract_id}, but this build creates {derived}. Nothing was staged; stop."
        ));
    }

    let units: u128 = expected.amount.to();
    let platform_units = units * expected.platformFee as u128 / 10_000;
    let protocol_units = units * TRUSTLESS_WORK_FEE_BPS / 10_000;
    let r = &expected.roles;
    let review = json!({
        "status": "awaiting_review",
        "network": format!("Arc Testnet ({ARC_TESTNET_CHAIN_ID})"),
        "contract_id": contract_id.to_string(),
        "signer": signer.to_string(),
        "engagement_id": expected.engagementId,
        "title": expected.title,
        "description": expected.description,
        "amount_usdc": usdc(units),
        "fees": {
            "platform_fee_percent": fee,
            "platform_fee_usdc": usdc(platform_units),
            "trustless_work_fee_percent": "0.3",
            "trustless_work_fee_usdc": usdc(protocol_units),
            "receiver_gets_usdc": usdc(units - platform_units - protocol_units),
            "note": "Fees are taken from the escrow when funds are released.",
        },
        "roles": {
            "approvers": r.approvers.iter().map(Address::to_string).collect::<Vec<_>>(),
            "service_providers": r.serviceProviders.iter().map(Address::to_string).collect::<Vec<_>>(),
            "release_signers": r.releaseSigners.iter().map(Address::to_string).collect::<Vec<_>>(),
            "dispute_resolvers": r.disputeResolvers.iter().map(Address::to_string).collect::<Vec<_>>(),
            "receiver": r.receiver.to_string(),
            "platform": r.platform.to_string(),
            "admin": r.admin.to_string(),
            "observers": r.observers.iter().map(Address::to_string).collect::<Vec<_>>(),
        },
        "milestones": expected.milestones.iter().map(|m| json!({
            "description": m.description,
            "approvals_target": m.approvals.target,
        })).collect::<Vec<_>>(),
        "transaction": {
            "to": FACTORY.to_string(),
            "nonce": hex_u64("nonce"),
            "gas": hex_u64("gas"),
            "unsigned": true,
        },
        "next_step": "Show every detail above, including contract_id. Call trustless_work_simulate_escrow with this contract_id only after the user confirms. Any rebuild creates a new contract_id that must be reviewed again.",
    });
    let build = ReviewedBuild {
        contract_id,
        signer,
        data: data.to_string(),
        created: Instant::now(),
    };
    Ok((review, build))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SimulateEscrowArgs {
    /// The contract_id the user reviewed and confirmed, from trustless_work_review_escrow.
    pub contract_id: String,
}

pub(crate) struct SimulateEscrow;

impl DynAomiTool for SimulateEscrow {
    type App = TrustlessWorkApp;
    type Args = SimulateEscrowArgs;
    const NAME: &'static str = "trustless_work_simulate_escrow";
    const DESCRIPTION: &'static str = "After the user confirms the details from trustless_work_review_escrow, stage that exact escrow-creation transaction and have the host simulate it on Arc Testnet. Takes the reviewed contract_id; refuses any address that is not the latest reviewed build. Does not sign or broadcast. The host injects [[SYSTEM:...]] prompts for the stage and simulate steps.";

    fn run_with_routes(
        _app: &TrustlessWorkApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let signer = arc_wallet(&ctx)?;
        let reviewed = args.contract_id.trim().parse::<Address>().ok();
        let build = {
            let mut builds = BUILDS.lock().map_err(|_| "build store poisoned")?;
            match builds.get(&ctx.session_id) {
                Some(b)
                    if Some(b.contract_id) == reviewed
                        && b.signer == signer
                        && b.created.elapsed() < BUILD_TTL =>
                {
                    builds.remove(&ctx.session_id)
                }
                _ => None,
            }
        }
        .ok_or_else(|| {
            format!(
                "{} is not the latest reviewed escrow build for this wallet, or it expired. Call trustless_work_review_escrow again and show the new details for review.",
                args.contract_id
            )
        })?;

        let stage = json!({
            "to": FACTORY.to_string(),
            "data": { "raw": build.data },
            "value": "0",
            "chain_id": ARC_TESTNET_CHAIN_ID,
            "description": format!("Create Trustless Work escrow {} on Arc Testnet", build.contract_id),
            "kind": "trustless_work_deploy_escrow",
        });
        ToolReturn::route(json!({
            "status": "staging_for_simulation",
            "contract_id": build.contract_id.to_string(),
            "network": format!("Arc Testnet ({ARC_TESTNET_CHAIN_ID})"),
        }))
        .next(|next| {
            next.add::<host::StageTx>(stage)
                .enforce(EnforcementPolicy::Stop, |enforce| {
                    enforce.add::<host::SimulateBatch>(json!({}));
                })
                .note("Stage these exact args; copy `data.raw` byte-for-byte. The host then simulates the escrow creation. Report the result. If simulation fails (for example a stale nonce), nothing was sent: call trustless_work_review_escrow again and show the new details for review. Do not call commit_txs unless the user separately asks to sign.");
        })
        .try_build()
        .map_err(|e| format!("[trustless-work] route build: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use aomi_sdk::testing::{TestCtxBuilder, run_tool};

    const SIGNER: Address = address!("0x1111111111111111111111111111111111111111");
    const CLIENT: Address = address!("0x2222222222222222222222222222222222222222");
    const PROVIDER: Address = address!("0x3333333333333333333333333333333333333333");
    const RESOLVER: Address = address!("0x4444444444444444444444444444444444444444");
    const PLATFORM: Address = address!("0x5555555555555555555555555555555555555555");
    const ADMIN: Address = address!("0x6666666666666666666666666666666666666666");

    fn ctx(tool: &str, session: &str) -> DynToolCallCtx {
        TestCtxBuilder::new(tool)
            .session_id(session)
            .attribute(
                "domain",
                json!({"evm": {"chain_id": ARC_TESTNET_CHAIN_ID, "address": SIGNER.to_string()}}),
            )
            .build()
    }

    fn agreement() -> Value {
        json!({
            "engagement_id": "TW-AOMI-001",
            "title": "Landing page build",
            "description": "Design and ship the landing page.",
            "roles": {
                "approvers": [CLIENT.to_string()],
                "service_providers": [PROVIDER.to_string()],
                "platform": PLATFORM.to_string(),
                "release_signers": [CLIENT.to_string()],
                "dispute_resolvers": [RESOLVER.to_string()],
                "receiver": PROVIDER.to_string(),
                "admin": ADMIN.to_string(),
            },
            "amount": "10",
            "platform_fee": 2,
            "milestones": [{"description": "Design"}, {"description": "Launch", "approvals_target": 1}],
        })
    }

    /// The escrow `agreement()` must deploy, as the contract stores it.
    fn expected() -> Escrow {
        let milestone = |d: &str| Milestone {
            description: d.into(),
            status: "pending".into(),
            evidence: String::new(),
            approvals: MilestoneApprovals {
                target: 1,
                approvalCount: 0,
                approvedBy: vec![],
            },
        };
        Escrow {
            engagementId: "TW-AOMI-001".into(),
            title: "Landing page build".into(),
            roles: Roles {
                approvers: vec![CLIENT],
                serviceProviders: vec![PROVIDER],
                platform: PLATFORM,
                releaseSigners: vec![CLIENT],
                disputeResolvers: vec![RESOLVER],
                receiver: PROVIDER,
                admin: ADMIN,
                observers: vec![],
            },
            description: "Design and ship the landing page.".into(),
            amount: U256::from(10_000_000u64),
            platformFee: 200,
            milestones: vec![milestone("Design"), milestone("Launch")],
            dispute: Dispute {
                isDisputed: false,
                reason: String::new(),
                resolved: false,
            },
            released: false,
            token: USDC,
            receiverMemo: 0,
        }
    }

    fn derive(salt: B256) -> Address {
        let mut init_code =
            alloy_primitives::hex!("3d602d80600a3d3981f3363d3d373d3d3d363d73").to_vec();
        init_code.extend_from_slice(IMPLEMENTATION.as_slice());
        init_code.extend_from_slice(&alloy_primitives::hex!("5af43d82803e903d91602b57fd5bf3"));
        FACTORY.create2(
            keccak256((SIGNER, salt).abi_encode()),
            keccak256(&init_code),
        )
    }

    /// A `/deploy` response for `escrow` with the given salt, shaped like the API's.
    fn deploy_response(escrow: Escrow, salt: B256) -> Value {
        let data = createEscrowCall {
            salt,
            escrowProperties: escrow,
        }
        .abi_encode();
        json!({
            "unsignedTransaction": "0x02",
            "transaction": {
                "from": SIGNER.to_string(),
                "to": FACTORY.to_string(),
                "data": alloy_primitives::hex::encode_prefixed(data),
                "value": "0x0",
                "chainId": "0x4cef52",
                "nonce": "0x5",
                "gas": "0x1d4c0",
                "maxFeePerGas": "0x4a817c800",
                "maxPriorityFeePerGas": "0x0",
                "type": "0x2",
            },
            "contractId": derive(salt).to_string(),
        })
    }

    #[test]
    fn rule_violations_stop_before_anything_is_built() {
        // No API key in the context: reaching the API would fail differently.
        let mut overlap = agreement();
        overlap["roles"]["dispute_resolvers"] = json!([CLIENT.to_string()]);
        let err =
            run_tool::<ReviewEscrow>(&TrustlessWorkApp, overlap, ctx("review", "s")).unwrap_err();
        assert!(
            err.starts_with("ESCROW_DISPUTE_RESOLVER_OVERLAPS_WITH_OTHER_ROLE (HTTP 422)"),
            "{err}"
        );
        assert!(err.contains("approver"), "{err}");

        for amount in [json!("10.1234567"), json!(10.1234567), json!("0.0000001")] {
            let mut precise = agreement();
            precise["amount"] = amount;
            let err = run_tool::<ReviewEscrow>(&TrustlessWorkApp, precise, ctx("review", "s"))
                .unwrap_err();
            assert!(err.starts_with("AMOUNT_PRECISION_UNSUPPORTED"), "{err}");
        }

        let mut wrong_chain = agreement();
        wrong_chain["amount"] = json!("10");
        let base_sepolia = TestCtxBuilder::new("review")
            .attribute(
                "domain",
                json!({"evm": {"chain_id": 84532, "address": SIGNER.to_string()}}),
            )
            .build();
        assert!(run_tool::<ReviewEscrow>(&TrustlessWorkApp, wrong_chain, base_sepolia).is_err());
    }

    #[test]
    fn amounts_stay_in_normal_units() {
        let exact = |v: Value, places| match decimal(&v, places) {
            Decimal::Exact(text, scaled) => Some((text, scaled)),
            _ => None,
        };
        assert_eq!(exact(json!("10"), 6), Some(("10".into(), 10_000_000)));
        assert_eq!(exact(json!(10), 6), Some(("10".into(), 10_000_000)));
        assert_eq!(
            exact(json!("0010.500000"), 6),
            Some(("10.5".into(), 10_500_000))
        );
        assert_eq!(exact(json!("0.000001"), 6), Some(("0.000001".into(), 1)));
        assert_eq!(exact(json!(2), 2), Some(("2".into(), 200)));
        assert_eq!(exact(json!("2.5"), 2), Some(("2.5".into(), 250)));
        assert!(matches!(
            decimal(&json!("10.1234567"), 6),
            Decimal::TooPrecise
        ));
        assert!(matches!(decimal(&json!("2.555"), 2), Decimal::TooPrecise));
        for bad in [
            json!("-1"),
            json!("1e6"),
            json!("10."),
            json!(""),
            json!(null),
            json!("1234567890"),
        ] {
            assert!(matches!(decimal(&bad, 6), Decimal::Malformed), "{bad}");
        }
        assert_eq!(usdc(9_770_000), "9.77");
        assert_eq!(usdc(30_000), "0.03");
        assert_eq!(usdc(1), "0.000001");
    }

    #[test]
    fn valid_agreement_is_reviewed_then_staged_and_simulated() {
        let salt = B256::repeat_byte(7);
        let response = deploy_response(expected(), salt);
        let (review, build) = check_build(&expected(), SIGNER, "2", &response).unwrap();
        assert_eq!(review["contract_id"], derive(salt).to_string());
        assert_eq!(review["amount_usdc"], "10");
        assert_eq!(review["fees"]["platform_fee_usdc"], "0.2");
        assert_eq!(review["fees"]["trustless_work_fee_usdc"], "0.03");
        assert_eq!(review["fees"]["receiver_gets_usdc"], "9.77");
        assert_eq!(review["milestones"].as_array().unwrap().len(), 2);
        assert_eq!(review["transaction"]["nonce"], 5);

        BUILDS.lock().unwrap().insert("valid".into(), build);
        let routed = run_tool::<SimulateEscrow>(
            &TrustlessWorkApp,
            json!({"contract_id": derive(salt).to_string()}),
            ctx("simulate", "valid"),
        )
        .unwrap();
        let routes = serde_json::to_value(&routed).unwrap()["__aomi_tool_routes"].clone();
        assert_eq!(routes.as_array().unwrap().len(), 1);
        assert_eq!(routes[0]["tool"], "stage_tx");
        assert_eq!(routes[0]["args"]["to"], FACTORY.to_string());
        assert_eq!(
            routes[0]["args"]["data"]["raw"],
            response["transaction"]["data"]
        );
        assert_eq!(routes[0]["args"]["chain_id"], ARC_TESTNET_CHAIN_ID);
        assert_eq!(routes[0]["enforcement"]["on_failure"], "stop");
        let steps = routes[0]["enforcement"]["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["tool"], "simulate_batch");
        assert!(!routes.to_string().contains("\"commit_txs\""));
    }

    #[test]
    fn builds_that_differ_from_the_agreement_are_refused() {
        let salt = B256::repeat_byte(7);
        let mut tampered: Vec<(&str, Value)> = Vec::new();

        let mut multiplied = expected();
        multiplied.amount = U256::from(10_000_000u64 * 1_000_000);
        tampered.push(("amount", deploy_response(multiplied, salt)));
        let mut fee = expected();
        fee.platformFee = 2;
        tampered.push(("platformFee", deploy_response(fee, salt)));
        let mut roles = expected();
        roles.roles.receiver = PLATFORM;
        tampered.push(("roles", deploy_response(roles, salt)));
        let mut milestones = expected();
        milestones.milestones.pop();
        tampered.push(("milestones", deploy_response(milestones, salt)));

        // The address from a different /deploy call never matches this build.
        let mut other_build = deploy_response(expected(), salt);
        other_build["contractId"] = json!(derive(B256::repeat_byte(8)).to_string());
        tampered.push(("creates", other_build));
        for (field, value) in [
            ("from", json!(PLATFORM.to_string())),
            ("to", json!(PLATFORM.to_string())),
            ("chainId", json!("0x1")),
            ("value", json!("0x1")),
            ("data", json!("0x095ea7b3")),
        ] {
            let mut response = deploy_response(expected(), salt);
            response["transaction"][field] = value;
            tampered.push((field, response));
        }

        for (field, response) in tampered {
            let err = check_build(&expected(), SIGNER, "2", &response).err();
            assert!(err.is_some(), "{field} was accepted");
            if field != "data" {
                assert!(err.unwrap().contains(field), "{field}");
            }
        }
    }

    #[test]
    fn only_the_latest_reviewed_address_can_be_simulated() {
        let (first_salt, second_salt) = (B256::repeat_byte(1), B256::repeat_byte(2));
        let (_, first) = check_build(
            &expected(),
            SIGNER,
            "2",
            &deploy_response(expected(), first_salt),
        )
        .unwrap();
        let (_, second) = check_build(
            &expected(),
            SIGNER,
            "2",
            &deploy_response(expected(), second_salt),
        )
        .unwrap();
        BUILDS.lock().unwrap().insert("rebuild".into(), first);
        BUILDS.lock().unwrap().insert("rebuild".into(), second);

        let simulate = |address: Address, session: &str| {
            run_tool::<SimulateEscrow>(
                &TrustlessWorkApp,
                json!({"contract_id": address.to_string()}),
                ctx("simulate", session),
            )
        };
        // A rebuild replaced the first address; it must be reviewed again.
        assert!(simulate(derive(first_salt), "rebuild").is_err());
        assert!(simulate(derive(second_salt), "other-session").is_err());
        assert!(simulate(derive(second_salt), "rebuild").is_ok());
        // Each reviewed build is simulated once.
        assert!(simulate(derive(second_salt), "rebuild").is_err());
    }

    fn rpc(method: &str, params: Value) -> Value {
        let url = std::env::var("ARC_RPC_URL").unwrap_or("https://arc-testnet.drpc.org".into());
        let response: Value = reqwest::blocking::Client::new()
            .post(url)
            .json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
            .send()
            .unwrap()
            .json()
            .unwrap();
        assert!(response["error"].is_null(), "{method}: {response}");
        response["result"].clone()
    }

    /// Read-only: checks the constants and the address derivation against the
    /// deployed factory, then simulates a deploy with `eth_call`.
    #[test]
    #[ignore = "reads Arc Testnet"]
    fn live_factory_derivation_and_simulation() {
        let call = |data: Vec<u8>, from: Option<Address>| {
            let mut tx = json!({"to": FACTORY.to_string(), "data": alloy_primitives::hex::encode_prefixed(data)});
            if let Some(from) = from {
                tx["from"] = json!(from.to_string());
            }
            let word = rpc("eth_call", json!([tx, "latest"]));
            Address::from_word(word.as_str().unwrap().parse().unwrap())
        };
        sol! {
            function implementation() returns (address);
            function predictEscrowAddress(address creator, bytes32 salt) returns (address);
        }
        assert_eq!(
            call(implementationCall {}.abi_encode(), None),
            IMPLEMENTATION
        );
        let salt = keccak256(format!("{:?}", Instant::now()));
        let predicted = call(
            predictEscrowAddressCall {
                creator: SIGNER,
                salt,
            }
            .abi_encode(),
            None,
        );
        assert_eq!(predicted, derive(salt));

        let response = deploy_response(expected(), salt);
        let data = alloy_primitives::hex::decode(response["transaction"]["data"].as_str().unwrap())
            .unwrap();
        assert_eq!(call(data.clone(), Some(SIGNER)), derive(salt));

        let mut overlap = expected();
        overlap.roles.disputeResolvers = vec![CLIENT];
        let reverted = reqwest::blocking::Client::new()
            .post(std::env::var("ARC_RPC_URL").unwrap_or("https://arc-testnet.drpc.org".into()))
            .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "eth_call", "params": [{
                "from": SIGNER.to_string(),
                "to": FACTORY.to_string(),
                "data": alloy_primitives::hex::encode_prefixed(createEscrowCall { salt, escrowProperties: overlap }.abi_encode()),
            }, "latest"]}))
            .send()
            .unwrap()
            .json::<Value>()
            .unwrap();
        assert!(
            !reverted["error"].is_null(),
            "overlapping resolver deployed: {reverted}"
        );
    }

    /// Calls the real API with TRUSTLESS_WORK_API_KEY and the wallet in
    /// TRUSTLESS_WORK_SIGNER, then simulates the reviewed build with eth_call.
    #[test]
    #[ignore = "needs TRUSTLESS_WORK_API_KEY"]
    fn live_api_build_review_and_simulation() {
        let key = std::env::var("TRUSTLESS_WORK_API_KEY").unwrap();
        let signer = std::env::var("TRUSTLESS_WORK_SIGNER").map_or(SIGNER, |s| s.parse().unwrap());
        let ctx = |session: &str| {
            TestCtxBuilder::new("review")
                .session_id(session)
                .secret("TRUSTLESS_WORK_API_KEY", key.clone())
                .attribute(
                    "domain",
                    json!({"evm": {"chain_id": ARC_TESTNET_CHAIN_ID, "address": signer.to_string()}}),
                )
                .build()
        };
        let review = run_tool::<ReviewEscrow>(&TrustlessWorkApp, agreement(), ctx("live"))
            .unwrap()
            .into_value();
        println!("{}", serde_json::to_string_pretty(&review).unwrap());
        assert_eq!(review["amount_usdc"], "10");
        let data = BUILDS.lock().unwrap()["live"].data.clone();
        let created = rpc(
            "eth_call",
            json!([{"from": signer.to_string(), "to": FACTORY.to_string(), "data": data}, "latest"]),
        );
        let created = Address::from_word(created.as_str().unwrap().parse().unwrap());
        assert_eq!(created.to_string(), review["contract_id"]);

        // A second build is a new address, which replaces the first.
        let again = run_tool::<ReviewEscrow>(&TrustlessWorkApp, agreement(), ctx("live"))
            .unwrap()
            .into_value();
        assert_ne!(again["contract_id"], review["contract_id"]);

        // The API itself must refuse both rule violations.
        let api = |body: Value| {
            let response = reqwest::blocking::Client::new()
                .post(format!("{API_URL}/escrow/single-release/v2/deploy"))
                .header("x-api-key", &key)
                .json(&body)
                .send()
                .unwrap();
            (
                response.status().as_u16(),
                response.json::<Value>().unwrap(),
            )
        };
        let body = |amount: Value, resolver: Address| {
            json!({
                "signer": signer.to_string(), "engagementId": "TW-AOMI-001", "title": "t", "description": "d",
                "roles": {"approvers": [CLIENT.to_string()], "serviceProviders": [PROVIDER.to_string()],
                    "platform": PLATFORM.to_string(), "releaseSigners": [CLIENT.to_string()],
                    "disputeResolvers": [resolver.to_string()], "receiver": PROVIDER.to_string(), "admin": ADMIN.to_string()},
                "amount": amount, "platformFee": 2, "milestones": [{"description": "Design"}],
            })
        };
        let (status, overlap) = api(body(json!(10), CLIENT));
        println!("overlap: {status} {overlap}");
        assert_eq!(
            (status, overlap["code"].as_str()),
            (
                422,
                Some("ESCROW_DISPUTE_RESOLVER_OVERLAPS_WITH_OTHER_ROLE")
            )
        );
        let (status, precise) = api(body(json!(10.1234567), RESOLVER));
        println!("precision: {status} {precise}");
        assert_eq!(precise["code"], "AMOUNT_PRECISION_UNSUPPORTED");
    }
}
