use crate::client::{
    CLOB_API_BASE, ClobApiCredentials, ClobAuthContext, ClobL1Auth, HEADER_POLY_ADDRESS,
    HEADER_POLY_API_KEY, HEADER_POLY_PASSPHRASE, HEADER_POLY_SIGNATURE, HEADER_POLY_TIMESTAMP,
    Market, PolymarketClient, PolymarketOrderPlan, PreparedPolymarketExchangeOrder,
    PreparedPolymarketOrder, SdkAuthedClobClient, TOKIO_RT, extract_outcome_token_ids,
    extract_yes_no_prices, fetch_clob_outcome_token_ids, normalize_side, normalize_yes_no,
};
use alloy::signers::{
    Error as AlloySignerError, UnsupportedSignerOperation, local::PrivateKeySigner,
};
use async_trait::async_trait;
use polymarket_client_sdk::{
    POLYGON, PRIVATE_KEY_VAR,
    auth::{Credentials as SdkCredentials, LocalSigner, Signer as SdkSigner},
    clob::{
        self,
        types::{Amount, OrderType, Side, SignableOrder, SignatureType},
    },
    contract_config,
    types::{Address, B256, Decimal, Signature, U256},
};
use serde_json::{Value, json};
use std::{env, str::FromStr};

const CLOB_AUTH_MESSAGE: &str = "This message attests that I control the given wallet";
const EXCHANGE_EIP712_NAME: &str = "Polymarket CTF Exchange";
const EXCHANGE_EIP712_VERSION: &str = "1";
const CLOB_AUTH_EIP712_NAME: &str = "ClobAuthDomain";
const CLOB_AUTH_EIP712_VERSION: &str = "1";

type RuntimeSigner = PrivateKeySigner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionTarget {
    DirectSdk,
    Wallet,
}

impl ExecutionTarget {
    fn label(self) -> &'static str {
        match self {
            Self::DirectSdk => "DIRECT_SDK",
            Self::Wallet => "WALLET",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderKind {
    Limit,
    Market,
}

impl OrderKind {
    fn label(self) -> &'static str {
        match self {
            Self::Limit => "LIMIT",
            Self::Market => "MARKET",
        }
    }
}

#[derive(Debug, Clone)]
struct OrderPlanBuilder<'a> {
    market: &'a Market,
    market_id_or_slug: &'a str,
    outcome: &'a str,
    side: Option<&'a str>,
    size_usd: Option<f64>,
    shares: Option<f64>,
    limit_price: Option<f64>,
    order_type: Option<&'a str>,
    post_only: Option<bool>,
    signature_type: Option<&'a str>,
    funder: Option<&'a str>,
    execution_target: ExecutionTarget,
    wallet_address: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub(crate) struct BuildOrderPlanRequest<'a> {
    pub(crate) market: &'a Market,
    pub(crate) market_id_or_slug: &'a str,
    pub(crate) outcome: &'a str,
    pub(crate) side: Option<&'a str>,
    pub(crate) size_usd: Option<f64>,
    pub(crate) shares: Option<f64>,
    pub(crate) limit_price: Option<f64>,
    pub(crate) order_type: Option<&'a str>,
    pub(crate) post_only: Option<bool>,
    pub(crate) signature_type: Option<&'a str>,
    pub(crate) funder: Option<&'a str>,
    pub(crate) execution_mode: &'a str,
    pub(crate) wallet_address: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct OrderPreviewNumbers {
    amount: Option<Decimal>,
    amount_kind: Option<&'static str>,
    price: Option<Decimal>,
    size: Option<Decimal>,
    reference_price: Option<Decimal>,
    estimated_shares: Option<Decimal>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct PreparedWalletStage {
    prepared_order: PreparedPolymarketOrder,
}

#[derive(Debug, Clone)]
struct DirectOrderExecutor {
    client: SdkAuthedClobClient,
    signer: RuntimeSigner,
}

#[derive(Debug, Clone)]
struct AddressOnlySigner {
    address: Address,
    chain_id: Option<u64>,
}

impl AddressOnlySigner {
    fn new(address: Address, chain_id: u64) -> Self {
        Self {
            address,
            chain_id: Some(chain_id),
        }
    }
}

#[async_trait]
impl SdkSigner for AddressOnlySigner {
    async fn sign_hash(&self, _hash: &B256) -> Result<Signature, AlloySignerError> {
        Err(AlloySignerError::UnsupportedOperation(
            UnsupportedSignerOperation::SignHash,
        ))
    }

    fn address(&self) -> Address {
        self.address
    }

    fn chain_id(&self) -> Option<u64> {
        self.chain_id
    }

    fn set_chain_id(&mut self, chain_id: Option<u64>) {
        self.chain_id = chain_id;
    }
}

impl<'a> OrderPlanBuilder<'a> {
    fn build(&self) -> Result<PolymarketOrderPlan, String> {
        let outcome = normalize_yes_no(self.outcome)?;
        let side = normalize_side(self.side)?;
        let token_id = self.resolve_token_id(&outcome)?;
        let reference_price = self.reference_price(&outcome)?;
        let (signature_type, signature_type_label) =
            resolve_sdk_signature_type(self.signature_type)?;
        let funder = self.funder.map(validate_address_string).transpose()?;
        let order_kind = if self.limit_price.is_some() {
            OrderKind::Limit
        } else {
            OrderKind::Market
        };
        let order_type = resolve_order_type(order_kind, self.order_type)?;
        let preview = self.build_preview_numbers(order_kind, &side, reference_price)?;

        if matches!(signature_type, SignatureType::Eoa) && funder.is_some() {
            return Err("funder cannot be set when signature_type is eoa".to_string());
        }

        Ok(PolymarketOrderPlan {
            market_id_or_slug: self.market_id_or_slug.to_string(),
            market_id: self.market.id.clone(),
            slug: self.market.slug.clone(),
            condition_id: self.market.condition_id.clone(),
            question: self.market.question.clone(),
            close_time: self.market.end_date.clone(),
            token_id,
            outcome,
            side,
            execution_mode: self.execution_target.label().to_string(),
            order_kind: order_kind.label().to_string(),
            amount: preview.amount.map(format_decimal),
            amount_kind: preview.amount_kind.map(str::to_string),
            price: preview.price.map(format_decimal),
            size: preview.size.map(format_decimal),
            reference_price: preview.reference_price.map(format_decimal),
            estimated_shares: preview.estimated_shares.map(format_decimal),
            order_type: order_type.to_string(),
            post_only: matches!(order_kind, OrderKind::Limit) && self.post_only.unwrap_or(false),
            signature_type: signature_type_label,
            funder,
            wallet_address: self.wallet_address.map(str::to_string),
            warnings: preview.warnings,
        })
    }

    fn resolve_token_id(&self, outcome: &str) -> Result<String, String> {
        let (mut yes_token_id, mut no_token_id) = extract_outcome_token_ids(self.market);
        if (yes_token_id.is_none() || no_token_id.is_none())
            && let Some(condition_id) = self.market.condition_id.as_deref()
            && let Ok((sdk_yes, sdk_no, _)) = fetch_clob_outcome_token_ids(condition_id)
        {
            if yes_token_id.is_none() {
                yes_token_id = sdk_yes;
            }
            if no_token_id.is_none() {
                no_token_id = sdk_no;
            }
        }

        match outcome {
            "YES" => yes_token_id.ok_or_else(|| {
                "unable to resolve the YES token_id for this market; fetch market details first"
                    .to_string()
            }),
            "NO" => no_token_id.ok_or_else(|| {
                "unable to resolve the NO token_id for this market; fetch market details first"
                    .to_string()
            }),
            _ => Err("outcome must be YES or NO".to_string()),
        }
    }

    fn reference_price(&self, outcome: &str) -> Result<Option<Decimal>, String> {
        let (yes_price, no_price) = extract_yes_no_prices(self.market);
        let price = match outcome {
            "YES" => yes_price,
            "NO" => no_price,
            _ => None,
        };
        price
            .map(|value| decimal_from_f64(value, "reference_price"))
            .transpose()
    }

    fn build_preview_numbers(
        &self,
        order_kind: OrderKind,
        side: &str,
        reference_price: Option<Decimal>,
    ) -> Result<OrderPreviewNumbers, String> {
        let mut warnings = Vec::new();

        match order_kind {
            OrderKind::Limit => {
                let price = decimal_from_f64(
                    self.limit_price
                        .ok_or_else(|| "limit_price is required for limit orders".to_string())?,
                    "limit_price",
                )?;

                if price <= Decimal::ZERO || price >= Decimal::ONE {
                    return Err("limit_price must be between 0 and 1".to_string());
                }

                let size = if let Some(shares) = self.shares {
                    decimal_from_f64(shares, "shares")?
                } else if let Some(notional) = self.size_usd {
                    let notional = decimal_from_f64(notional, "size_usd")?;
                    let exact = notional / price;
                    let truncated = exact.trunc_with_scale(2).normalize();
                    if truncated != exact.normalize() {
                        warnings.push(
                            "Converted size_usd into shares for a limit order and truncated to 2 decimal places."
                                .to_string(),
                        );
                    } else {
                        warnings.push(
                            "Converted size_usd into shares for a limit order using the provided limit_price."
                                .to_string(),
                        );
                    }
                    truncated
                } else {
                    return Err(
                        "limit orders require shares, or size_usd together with limit_price"
                            .to_string(),
                    );
                };

                if size <= Decimal::ZERO {
                    return Err("limit order size must be positive".to_string());
                }

                Ok(OrderPreviewNumbers {
                    amount: None,
                    amount_kind: None,
                    price: Some(price),
                    size: Some(size),
                    reference_price,
                    estimated_shares: Some(size),
                    warnings,
                })
            }
            OrderKind::Market => {
                if side == "SELL" {
                    let shares = decimal_from_f64(
                        self.shares.ok_or_else(|| {
                            "market SELL orders require shares; do not pass only size_usd"
                                .to_string()
                        })?,
                        "shares",
                    )?;
                    if shares <= Decimal::ZERO {
                        return Err("market order shares must be positive".to_string());
                    }

                    return Ok(OrderPreviewNumbers {
                        amount: Some(shares),
                        amount_kind: Some("SHARES"),
                        price: None,
                        size: None,
                        reference_price,
                        estimated_shares: Some(shares),
                        warnings,
                    });
                }

                if let Some(size_usd) = self.size_usd {
                    let amount = decimal_from_f64(size_usd, "size_usd")?;
                    if amount <= Decimal::ZERO {
                        return Err("market order amount must be positive".to_string());
                    }

                    let estimated_shares = reference_price.and_then(|price| {
                        if price > Decimal::ZERO {
                            Some((amount / price).normalize())
                        } else {
                            None
                        }
                    });

                    return Ok(OrderPreviewNumbers {
                        amount: Some(amount),
                        amount_kind: Some("USDC"),
                        price: None,
                        size: None,
                        reference_price,
                        estimated_shares,
                        warnings,
                    });
                }

                if let Some(shares) = self.shares {
                    let amount = decimal_from_f64(shares, "shares")?;
                    if amount <= Decimal::ZERO {
                        return Err("market order shares must be positive".to_string());
                    }

                    warnings.push(
                        "Using a BUY market order sized in shares; filled USDC notional may vary with book depth."
                            .to_string(),
                    );

                    return Ok(OrderPreviewNumbers {
                        amount: Some(amount),
                        amount_kind: Some("SHARES"),
                        price: None,
                        size: None,
                        reference_price,
                        estimated_shares: Some(amount),
                        warnings,
                    });
                }

                Err(
                    "market BUY orders require size_usd, or explicit shares if you intentionally want share-sized execution"
                        .to_string(),
                )
            }
        }
    }
}

impl DirectOrderExecutor {
    fn new(plan: &PolymarketOrderPlan, private_key: Option<&str>) -> Result<Self, String> {
        let signer = resolve_runtime_signer(private_key)?;
        let client = authenticate_with_signer(
            &signer,
            None,
            plan.signature_type.as_str(),
            plan.funder.as_deref(),
        )?;
        Ok(Self { client, signer })
    }

    fn submit(self, plan: &PolymarketOrderPlan) -> Result<Value, String> {
        let signable_order = build_signable_order_with_client(&self.client, plan)?;
        let signed_order = TOKIO_RT
            .block_on(self.client.sign(&self.signer, signable_order))
            .map_err(|e| format!("failed to sign Polymarket order: {e}"))?;
        let response = TOKIO_RT
            .block_on(self.client.post_order(signed_order))
            .map_err(|e| format!("failed to submit Polymarket order: {e}"))?;

        Ok(format_sdk_submit_result("DIRECT_SDK", plan, response))
    }
}

impl PreparedWalletStage {
    fn new(
        plan: &PolymarketOrderPlan,
        clob_auth: &ClobAuthContext,
        clob_l1_signature: &str,
    ) -> Result<Self, String> {
        let credentials = bootstrap_wallet_credentials(clob_auth, clob_l1_signature)?;
        let client = authenticate_wallet_client(
            clob_auth.address.as_str(),
            &credentials,
            plan.signature_type.as_str(),
            plan.funder.as_deref(),
        )?;
        let signable_order = build_signable_order_with_client(&client, plan)?;
        let prepared_order = build_prepared_order(&client, signable_order, plan)?;

        Ok(Self { prepared_order })
    }
}

pub(crate) fn has_configured_polymarket_private_key() -> bool {
    env::var(PRIVATE_KEY_VAR)
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
}

pub(crate) fn determine_polymarket_execution(
    connected_wallet: Option<&str>,
) -> Result<(String, Option<String>), String> {
    if has_configured_polymarket_private_key() {
        return Ok((ExecutionTarget::DirectSdk.label().to_string(), None));
    }

    let Some(wallet_address) = connected_wallet
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(
            "Polymarket execution requires either POLYMARKET_PRIVATE_KEY in the runtime or a connected wallet address for wallet-signing mode."
                .to_string(),
        );
    };
    validate_address_string(wallet_address)?;

    Ok((
        ExecutionTarget::Wallet.label().to_string(),
        Some(wallet_address.to_string()),
    ))
}

pub(crate) fn build_polymarket_order_plan_from_market(
    request: BuildOrderPlanRequest<'_>,
) -> Result<PolymarketOrderPlan, String> {
    let execution_target = match request.execution_mode {
        "DIRECT_SDK" => ExecutionTarget::DirectSdk,
        "WALLET" => ExecutionTarget::Wallet,
        other => return Err(format!("unsupported execution mode: {other}")),
    };

    OrderPlanBuilder {
        market: request.market,
        market_id_or_slug: request.market_id_or_slug,
        outcome: request.outcome,
        side: request.side,
        size_usd: request.size_usd,
        shares: request.shares,
        limit_price: request.limit_price,
        order_type: request.order_type,
        post_only: request.post_only,
        signature_type: request.signature_type,
        funder: request.funder,
        execution_target,
        wallet_address: request.wallet_address,
    }
    .build()
}

pub(crate) fn submit_direct_order_plan(
    plan: &PolymarketOrderPlan,
    private_key: Option<&str>,
) -> Result<Value, String> {
    if plan.execution_mode != ExecutionTarget::DirectSdk.label() {
        return Err("submit_direct_order_plan requires a DIRECT_SDK order plan".to_string());
    }

    DirectOrderExecutor::new(plan, private_key)?.submit(plan)
}

pub(crate) fn build_clob_auth_context(address: &str) -> ClobAuthContext {
    let timestamp = TOKIO_RT
        .block_on(clob::Client::default().server_time())
        .map(|value| value.to_string())
        .unwrap_or_else(|_| PolymarketClient::now_unix_timestamp());

    ClobAuthContext {
        address: address.to_string(),
        timestamp,
        nonce: "0".to_string(),
    }
}

pub(crate) fn build_clob_auth_typed_data(context: &ClobAuthContext) -> Value {
    json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
            ],
            "ClobAuth": [
                {"name": "address", "type": "address"},
                {"name": "timestamp", "type": "string"},
                {"name": "nonce", "type": "uint256"},
                {"name": "message", "type": "string"},
            ]
        },
        "primaryType": "ClobAuth",
        "domain": {
            "name": CLOB_AUTH_EIP712_NAME,
            "version": CLOB_AUTH_EIP712_VERSION,
            "chainId": POLYGON,
        },
        "message": {
            "address": context.address,
            "timestamp": context.timestamp,
            "nonce": context.nonce,
            "message": CLOB_AUTH_MESSAGE,
        }
    })
}

pub(crate) fn prepare_wallet_order_signature(
    plan: &PolymarketOrderPlan,
    clob_auth: &ClobAuthContext,
    clob_l1_signature: &str,
) -> Result<(PreparedPolymarketOrder, Value, Option<Address>), String> {
    if plan.execution_mode != ExecutionTarget::Wallet.label() {
        return Err("wallet preparation requires a WALLET order plan".to_string());
    }

    let prepared = PreparedWalletStage::new(plan, clob_auth, clob_l1_signature)?;
    let funder_address = Address::from_str(&prepared.prepared_order.order.maker).ok();
    let typed_data = build_order_typed_data(&prepared.prepared_order);

    Ok((prepared.prepared_order, typed_data, funder_address))
}

pub(crate) fn build_order_typed_data(prepared_order: &PreparedPolymarketOrder) -> Value {
    json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"},
            ],
            "Order": [
                {"name": "salt", "type": "uint256"},
                {"name": "maker", "type": "address"},
                {"name": "signer", "type": "address"},
                {"name": "taker", "type": "address"},
                {"name": "tokenId", "type": "uint256"},
                {"name": "makerAmount", "type": "uint256"},
                {"name": "takerAmount", "type": "uint256"},
                {"name": "expiration", "type": "uint256"},
                {"name": "nonce", "type": "uint256"},
                {"name": "feeRateBps", "type": "uint256"},
                {"name": "side", "type": "uint8"},
                {"name": "signatureType", "type": "uint8"},
            ],
        },
        "primaryType": "Order",
        "domain": {
            "name": EXCHANGE_EIP712_NAME,
            "version": EXCHANGE_EIP712_VERSION,
            "chainId": POLYGON,
            "verifyingContract": prepared_order.verifying_contract,
        },
        "message": {
            "salt": prepared_order.order.salt.to_string(),
            "maker": prepared_order.order.maker,
            "signer": prepared_order.order.signer,
            "taker": prepared_order.order.taker,
            "tokenId": prepared_order.order.token_id,
            "makerAmount": prepared_order.order.maker_amount,
            "takerAmount": prepared_order.order.taker_amount,
            "expiration": prepared_order.order.expiration,
            "nonce": prepared_order.order.nonce,
            "feeRateBps": prepared_order.order.fee_rate_bps,
            "side": prepared_order.order.side_index,
            "signatureType": prepared_order.order.signature_type,
        }
    })
}

pub(crate) fn build_prepared_order_description(plan: &PolymarketOrderPlan) -> String {
    let question = plan
        .question
        .clone()
        .unwrap_or_else(|| "market".to_string());
    match plan.order_kind.as_str() {
        "LIMIT" => format!(
            "Sign Polymarket limit order: {} {} at {} on {}",
            plan.side,
            plan.outcome,
            plan.price.as_deref().unwrap_or("?"),
            question
        ),
        _ => format!(
            "Sign Polymarket market order: {} {} on {}",
            plan.side, plan.outcome, question
        ),
    }
}

pub(crate) fn submit_wallet_signed_order(
    plan: &PolymarketOrderPlan,
    clob_auth: &ClobAuthContext,
    clob_l1_signature: &str,
    prepared_order: &PreparedPolymarketOrder,
    order_signature: &str,
) -> Result<Value, String> {
    if plan.execution_mode != ExecutionTarget::Wallet.label() {
        return Err("wallet order submission requires a WALLET order plan".to_string());
    }

    validate_prefixed_hex(order_signature, "order_signature")?;
    let credentials = bootstrap_wallet_credentials(clob_auth, clob_l1_signature)?;
    let response = post_signed_order_with_credentials(
        clob_auth.address.as_str(),
        &credentials,
        prepared_order,
        order_signature,
    )?;

    Ok(json!({
        "source": "polymarket",
        "execution_mode": "WALLET",
        "submitted": response.get("success").and_then(Value::as_bool).unwrap_or(false),
        "order_id": response.get("orderID").cloned().or_else(|| response.get("order_id").cloned()),
        "result": response,
        "order_plan": plan,
    }))
}

pub(crate) fn resolve_sdk_signature_type(
    value: Option<&str>,
) -> Result<(SignatureType, String), String> {
    let normalized = value.map(|raw| raw.trim().to_ascii_lowercase());
    match normalized.as_deref() {
        None | Some("") | Some("proxy") => Ok((SignatureType::Proxy, "proxy".to_string())),
        Some("eoa") => Ok((SignatureType::Eoa, "eoa".to_string())),
        Some("gnosis-safe") | Some("gnosis_safe") | Some("safe") => {
            Ok((SignatureType::GnosisSafe, "gnosis-safe".to_string()))
        }
        Some(other) => Err(format!(
            "unsupported signature_type `{other}`; expected proxy, eoa, or gnosis-safe"
        )),
    }
}

fn resolve_order_type(order_kind: OrderKind, value: Option<&str>) -> Result<OrderType, String> {
    let default = match order_kind {
        OrderKind::Limit => "GTC",
        OrderKind::Market => "FAK",
    };
    let normalized = value.unwrap_or(default).trim().to_ascii_uppercase();
    let order_type = match normalized.as_str() {
        "GTC" => OrderType::GTC,
        "FOK" => OrderType::FOK,
        "GTD" => OrderType::GTD,
        "FAK" => OrderType::FAK,
        other => return Err(format!("unsupported order_type `{other}`")),
    };

    match order_kind {
        OrderKind::Limit => Ok(order_type),
        OrderKind::Market if matches!(order_type, OrderType::FAK | OrderType::FOK) => {
            Ok(order_type)
        }
        OrderKind::Market => Err("market orders only support order_type FAK or FOK".to_string()),
    }
}

fn resolve_runtime_signer(private_key_override: Option<&str>) -> Result<RuntimeSigner, String> {
    let private_key = private_key_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            env::var(PRIVATE_KEY_VAR)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .ok_or_else(|| {
            format!(
                "missing Polymarket private key; pass private_key explicitly or set {PRIVATE_KEY_VAR}"
            )
        })?;

    LocalSigner::from_str(&private_key)
        .map_err(|e| format!("invalid Polymarket private key: {e}"))
        .map(|signer| signer.with_chain_id(Some(POLYGON)))
}

fn authenticate_with_signer<S: SdkSigner + Sync>(
    signer: &S,
    credentials: Option<SdkCredentials>,
    signature_type: &str,
    funder: Option<&str>,
) -> Result<SdkAuthedClobClient, String> {
    let (signature_type, _) = resolve_sdk_signature_type(Some(signature_type))?;
    let funder = funder.map(parse_address).transpose()?;

    TOKIO_RT
        .block_on(async {
            let mut builder = clob::Client::default()
                .authentication_builder(signer)
                .signature_type(signature_type);
            if let Some(credentials) = credentials {
                builder = builder.credentials(credentials);
            }
            if let Some(funder) = funder {
                builder = builder.funder(funder);
            }
            builder.authenticate().await
        })
        .map_err(|e| format!("failed to authenticate Polymarket client: {e}"))
}

fn authenticate_wallet_client(
    wallet_address: &str,
    credentials: &ClobApiCredentials,
    signature_type: &str,
    funder: Option<&str>,
) -> Result<SdkAuthedClobClient, String> {
    let signer = AddressOnlySigner::new(parse_address(wallet_address)?, POLYGON);
    let credentials = to_sdk_credentials(credentials)?;
    authenticate_with_signer(&signer, Some(credentials), signature_type, funder)
}

fn to_sdk_credentials(credentials: &ClobApiCredentials) -> Result<SdkCredentials, String> {
    let key = polymarket_client_sdk::auth::Uuid::parse_str(credentials.key.as_str())
        .map_err(|e| format!("invalid Polymarket api_key UUID `{}`: {e}", credentials.key))?;
    Ok(SdkCredentials::new(
        key,
        credentials.secret.clone(),
        credentials.passphrase.clone(),
    ))
}

fn build_signable_order_with_client(
    client: &SdkAuthedClobClient,
    plan: &PolymarketOrderPlan,
) -> Result<SignableOrder, String> {
    let token_id = U256::from_str(plan.token_id.as_str())
        .map_err(|e| format!("invalid token_id `{}`: {e}", plan.token_id))?;
    let side = parse_sdk_side(plan.side.as_str())?;
    let order_type = resolve_order_type_from_plan(plan)?;

    match plan.order_kind.as_str() {
        "LIMIT" => {
            let price = parse_plan_decimal(plan.price.as_deref(), "price")?;
            let size = parse_plan_decimal(plan.size.as_deref(), "size")?;
            TOKIO_RT
                .block_on(
                    client
                        .limit_order()
                        .token_id(token_id)
                        .side(side)
                        .price(price)
                        .size(size)
                        .order_type(order_type)
                        .post_only(plan.post_only)
                        .build(),
                )
                .map_err(|e| format!("failed to build limit order: {e}"))
        }
        "MARKET" => {
            let amount_value = parse_plan_decimal(plan.amount.as_deref(), "amount")?;
            let amount = match plan.amount_kind.as_deref() {
                Some("USDC") => Amount::usdc(amount_value)
                    .map_err(|e| format!("invalid USDC market amount: {e}"))?,
                Some("SHARES") => Amount::shares(amount_value)
                    .map_err(|e| format!("invalid share amount: {e}"))?,
                Some(other) => {
                    return Err(format!(
                        "unsupported market amount_kind `{other}`; expected USDC or SHARES"
                    ));
                }
                None => {
                    return Err("market order plan is missing amount_kind".to_string());
                }
            };

            TOKIO_RT
                .block_on(
                    client
                        .market_order()
                        .token_id(token_id)
                        .side(side)
                        .amount(amount)
                        .order_type(order_type)
                        .build(),
                )
                .map_err(|e| format!("failed to build market order: {e}"))
        }
        other => Err(format!("unsupported order_kind `{other}`")),
    }
}

fn build_prepared_order(
    client: &SdkAuthedClobClient,
    signable_order: SignableOrder,
    plan: &PolymarketOrderPlan,
) -> Result<PreparedPolymarketOrder, String> {
    let neg_risk = TOKIO_RT
        .block_on(client.neg_risk(signable_order.order.tokenId))
        .map_err(|e| format!("failed to fetch neg-risk market config: {e}"))?;
    let verifying_contract = contract_config(POLYGON, neg_risk.neg_risk)
        .ok_or_else(|| "missing Polymarket exchange contract config for Polygon".to_string())?
        .exchange;

    let side = parse_sdk_side_label(signable_order.order.side)?;
    let salt: u64 = signable_order
        .order
        .salt
        .try_into()
        .map_err(|e| format!("order salt does not fit in u64: {e}"))?;

    Ok(PreparedPolymarketOrder {
        order: PreparedPolymarketExchangeOrder {
            salt,
            maker: signable_order.order.maker.to_string(),
            signer: signable_order.order.signer.to_string(),
            taker: signable_order.order.taker.to_string(),
            token_id: signable_order.order.tokenId.to_string(),
            maker_amount: signable_order.order.makerAmount.to_string(),
            taker_amount: signable_order.order.takerAmount.to_string(),
            expiration: signable_order.order.expiration.to_string(),
            nonce: signable_order.order.nonce.to_string(),
            fee_rate_bps: signable_order.order.feeRateBps.to_string(),
            side,
            side_index: signable_order.order.side,
            signature_type: signable_order.order.signatureType,
        },
        order_type: plan.order_type.clone(),
        post_only: signable_order.post_only,
        verifying_contract: verifying_contract.to_string(),
    })
}

fn bootstrap_wallet_credentials(
    clob_auth: &ClobAuthContext,
    clob_l1_signature: &str,
) -> Result<ClobApiCredentials, String> {
    validate_address_string(clob_auth.address.as_str())?;
    validate_prefixed_hex(clob_l1_signature, "clob_l1_signature")?;

    let client = PolymarketClient::new()?;
    client.create_or_derive_api_credentials(&ClobL1Auth {
        address: clob_auth.address.clone(),
        signature: clob_l1_signature.to_string(),
        timestamp: clob_auth.timestamp.clone(),
        nonce: Some(clob_auth.nonce.clone()),
    })
}

fn post_signed_order_with_credentials(
    wallet_address: &str,
    credentials: &ClobApiCredentials,
    prepared_order: &PreparedPolymarketOrder,
    order_signature: &str,
) -> Result<Value, String> {
    let client = PolymarketClient::new()?;
    let url = format!("{CLOB_API_BASE}/order");
    let body = build_signed_order_body(credentials, prepared_order, order_signature);
    let body_string =
        serde_json::to_string(&body).map_err(|e| format!("failed to serialize order body: {e}"))?;
    let request_path = client.extract_request_path(&url)?;
    let timestamp = PolymarketClient::now_unix_timestamp();
    let l2_signature = client.build_l2_signature(
        credentials.secret.as_str(),
        timestamp.as_str(),
        "POST",
        request_path.as_str(),
        body_string.as_str(),
    )?;

    let (status, text) = TOKIO_RT.block_on(async move {
        let response = client
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header(HEADER_POLY_ADDRESS, wallet_address)
            .header(HEADER_POLY_API_KEY, credentials.key.as_str())
            .header(HEADER_POLY_PASSPHRASE, credentials.passphrase.as_str())
            .header(HEADER_POLY_TIMESTAMP, timestamp.as_str())
            .header(HEADER_POLY_SIGNATURE, l2_signature.as_str())
            .body(body_string)
            .send()
            .await
            .map_err(|e| format!("wallet order submission failed: {e}"))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("failed to read order submission response: {e}"))?;
        Ok::<_, String>((status, text))
    })?;

    if !status.is_success() {
        return Err(format!("wallet order submission failed {status}: {text}"));
    }

    serde_json::from_str(&text).map_err(|e| {
        format!("wallet order submission succeeded but returned invalid JSON: {e} (body: {text})")
    })
}

pub(crate) fn build_signed_order_body(
    credentials: &ClobApiCredentials,
    prepared_order: &PreparedPolymarketOrder,
    order_signature: &str,
) -> Value {
    let mut body = json!({
        "owner": credentials.key,
        "orderType": prepared_order.order_type,
        "order": {
            "salt": prepared_order.order.salt,
            "maker": prepared_order.order.maker,
            "signer": prepared_order.order.signer,
            "taker": prepared_order.order.taker,
            "tokenId": prepared_order.order.token_id,
            "makerAmount": prepared_order.order.maker_amount,
            "takerAmount": prepared_order.order.taker_amount,
            "expiration": prepared_order.order.expiration,
            "nonce": prepared_order.order.nonce,
            "feeRateBps": prepared_order.order.fee_rate_bps,
            "side": prepared_order.order.side,
            "signatureType": prepared_order.order.signature_type,
            "signature": order_signature,
        },
    });

    if let Some(post_only) = prepared_order.post_only {
        body["postOnly"] = Value::Bool(post_only);
    }

    body
}

fn format_sdk_submit_result(
    execution_mode: &str,
    plan: &PolymarketOrderPlan,
    response: polymarket_client_sdk::clob::types::response::PostOrderResponse,
) -> Value {
    json!({
        "source": "polymarket",
        "execution_mode": execution_mode,
        "submitted": response.success,
        "order_id": response.order_id,
        "status": response.status.to_string(),
        "error_msg": response.error_msg,
        "making_amount": response.making_amount.to_string(),
        "taking_amount": response.taking_amount.to_string(),
        "transaction_hashes": response.transaction_hashes.into_iter().map(|hash| hash.to_string()).collect::<Vec<_>>(),
        "trade_ids": response.trade_ids,
        "order_plan": plan,
    })
}

fn resolve_order_type_from_plan(plan: &PolymarketOrderPlan) -> Result<OrderType, String> {
    let order_kind = match plan.order_kind.as_str() {
        "LIMIT" => OrderKind::Limit,
        "MARKET" => OrderKind::Market,
        other => return Err(format!("unsupported order_kind `{other}`")),
    };
    resolve_order_type(order_kind, Some(plan.order_type.as_str()))
}

fn parse_sdk_side(value: &str) -> Result<Side, String> {
    match value {
        "BUY" => Ok(Side::Buy),
        "SELL" => Ok(Side::Sell),
        other => Err(format!("unsupported side `{other}`")),
    }
}

fn parse_sdk_side_label(side: u8) -> Result<String, String> {
    match side {
        0 => Ok("BUY".to_string()),
        1 => Ok("SELL".to_string()),
        other => Err(format!("unsupported side index `{other}`")),
    }
}

fn parse_plan_decimal(value: Option<&str>, field: &str) -> Result<Decimal, String> {
    let raw = value.ok_or_else(|| format!("order plan is missing `{field}`"))?;
    Decimal::from_str(raw).map_err(|e| format!("invalid {field} `{raw}`: {e}"))
}

fn parse_address(value: &str) -> Result<Address, String> {
    Address::from_str(value).map_err(|e| format!("invalid address `{value}`: {e}"))
}

fn validate_address_string(value: &str) -> Result<String, String> {
    parse_address(value).map(|_| value.to_string())
}

fn validate_prefixed_hex(value: &str, field: &str) -> Result<(), String> {
    if value.starts_with("0x") && value.len() > 2 {
        Ok(())
    } else {
        Err(format!("{field} must be a 0x-prefixed hex string"))
    }
}

fn decimal_from_f64(value: f64, field: &str) -> Result<Decimal, String> {
    if !value.is_finite() {
        return Err(format!("{field} must be a finite number"));
    }
    Decimal::from_str(&value.to_string()).map_err(|e| format!("invalid {field}: {e}"))
}

fn format_decimal(value: Decimal) -> String {
    value.normalize().to_string()
}
