#[allow(unused_imports)]
pub use progenitor_client::{ByteStream, ClientInfo, Error, ResponseValue};
#[allow(unused_imports)]
use progenitor_client::{ClientHooks, OperationInfo, RequestBuilderExt, encode_path};
/// Types used as operation parameters and responses.
#[allow(clippy::all)]
pub mod types {
    /// Error types.
    pub mod error {
        /// Error from a `TryFrom` or `FromStr` implementation.
        pub struct ConversionError(::std::borrow::Cow<'static, str>);
        impl ::std::error::Error for ConversionError {}
        impl ::std::fmt::Display for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl ::std::fmt::Debug for ConversionError {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
                ::std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl From<&'static str> for ConversionError {
            fn from(value: &'static str) -> Self {
                Self(value.into())
            }
        }
        impl From<String> for ConversionError {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    }
    ///`ApiError`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "error": {
    ///      "examples": [
    ///        "AppError"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ApiError {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub error: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub message: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ApiError {
        fn default() -> Self {
            Self {
                error: Default::default(),
                message: Default::default(),
            }
        }
    }
    ///`CreditEligibility`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "availableCredit": {
    ///      "type": "number"
    ///    },
    ///    "creditLevel": {
    ///      "type": "integer",
    ///      "maximum": 4.0,
    ///      "minimum": 0.0
    ///    },
    ///    "currentDebt": {
    ///      "type": "number"
    ///    },
    ///    "eligible": {
    ///      "type": "boolean"
    ///    },
    ///    "maxCredit": {
    ///      "description": "USDC, human-readable units",
    ///      "type": "number"
    ///    },
    ///    "rateBps": {
    ///      "description": "Annual rate in basis points",
    ///      "type": "integer"
    ///    },
    ///    "reason": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct CreditEligibility {
        #[serde(
            rename = "availableCredit",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub available_credit: ::std::option::Option<f64>,
        #[serde(
            rename = "creditLevel",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub credit_level: ::std::option::Option<i64>,
        #[serde(
            rename = "currentDebt",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub current_debt: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub eligible: ::std::option::Option<bool>,
        ///USDC, human-readable units
        #[serde(
            rename = "maxCredit",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub max_credit: ::std::option::Option<f64>,
        ///Annual rate in basis points
        #[serde(
            rename = "rateBps",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub rate_bps: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub reason: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for CreditEligibility {
        fn default() -> Self {
            Self {
                available_credit: Default::default(),
                credit_level: Default::default(),
                current_debt: Default::default(),
                eligible: Default::default(),
                max_credit: Default::default(),
                rate_bps: Default::default(),
                reason: Default::default(),
            }
        }
    }
    ///`CreditLine`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "accruedInterest": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "active": {
    ///      "type": "boolean"
    ///    },
    ///    "healthFactor": {
    ///      "description": "Health factor in basis points (15000 = 150%)",
    ///      "type": "integer"
    ///    },
    ///    "openedAt": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "principal": {
    ///      "description": "USDC lamports (6 decimals)",
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "rateBps": {
    ///      "type": "integer"
    ///    },
    ///    "totalOwed": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct CreditLine {
        #[serde(
            rename = "accruedInterest",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub accrued_interest: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub active: ::std::option::Option<bool>,
        ///Health factor in basis points (15000 = 150%)
        #[serde(
            rename = "healthFactor",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub health_factor: ::std::option::Option<i64>,
        #[serde(
            rename = "openedAt",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub opened_at: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
        ///USDC lamports (6 decimals)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub principal: ::std::option::Option<i64>,
        #[serde(
            rename = "rateBps",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub rate_bps: ::std::option::Option<i64>,
        #[serde(
            rename = "totalOwed",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub total_owed: ::std::option::Option<i64>,
    }
    impl ::std::default::Default for CreditLine {
        fn default() -> Self {
            Self {
                accrued_interest: Default::default(),
                active: Default::default(),
                health_factor: Default::default(),
                opened_at: Default::default(),
                principal: Default::default(),
                rate_bps: Default::default(),
                total_owed: Default::default(),
            }
        }
    }
    ///`DeployAgentRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "agent",
    ///    "owner"
    ///  ],
    ///  "properties": {
    ///    "agent": {
    ///      "description": "Agent's Solana pubkey (base58).",
    ///      "type": "string"
    ///    },
    ///    "agentType": {
    ///      "description": "0 = Trader, 1 = Service, 2 = Hybrid.",
    ///      "type": "integer",
    ///      "maximum": 2.0,
    ///      "minimum": 0.0
    ///    },
    ///    "name": {
    ///      "description": "Display name (1-32 chars). Defaults to \"krexa-agent\".",
    ///      "type": "string"
    ///    },
    ///    "owner": {
    ///      "description": "Owner wallet pubkey (base58) — fee payer and authority. May\nbe the same as `agent` for self-custodied agents.\n",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct DeployAgentRequest {
        ///Agent's Solana pubkey (base58).
        pub agent: ::std::string::String,
        ///0 = Trader, 1 = Service, 2 = Hybrid.
        #[serde(
            rename = "agentType",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub agent_type: ::std::option::Option<i64>,
        ///Display name (1-32 chars). Defaults to "krexa-agent".
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        /**Owner wallet pubkey (base58) — fee payer and authority. May
        be the same as `agent` for self-custodied agents.
        */
        pub owner: ::std::string::String,
    }
    ///`DeployAgentResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "encoding": {
    ///      "description": "Encoding of `transaction`. Always `\"base64\"` when present.",
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "status": {
    ///      "description": "`ready` if a new deploy tx is returned; `exists` if the agent is already deployed.",
    ///      "type": "string"
    ///    },
    ///    "steps": {
    ///      "description": "The on-chain steps the tx performs (informational).",
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "transaction": {
    ///      "description": "Base64-encoded transaction with the oracle's KYA signature\nalready attached. Null when `status == \"exists\"`. Caller\nadds the agent/owner signature(s) and submits.\n",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct DeployAgentResponse {
        #[serde(
            rename = "agentPubkey",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub agent_pubkey: ::std::option::Option<::std::string::String>,
        ///Encoding of `transaction`. Always `"base64"` when present.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub encoding: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub message: ::std::option::Option<::std::string::String>,
        ///`ready` if a new deploy tx is returned; `exists` if the agent is already deployed.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<::std::string::String>,
        ///The on-chain steps the tx performs (informational).
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub steps: ::std::vec::Vec<::std::string::String>,
        /**Base64-encoded transaction with the oracle's KYA signature
        already attached. Null when `status == "exists"`. Caller
        adds the agent/owner signature(s) and submits.
        */
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub transaction: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for DeployAgentResponse {
        fn default() -> Self {
            Self {
                agent_pubkey: Default::default(),
                encoding: Default::default(),
                message: Default::default(),
                status: Default::default(),
                steps: Default::default(),
                transaction: Default::default(),
            }
        }
    }
    ///`FaucetRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "recipient"
    ///  ],
    ///  "properties": {
    ///    "amountUsdc": {
    ///      "default": 100,
    ///      "type": "number",
    ///      "maximum": 100.0
    ///    },
    ///    "recipient": {
    ///      "description": "Solana address (base58)",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct FaucetRequest {
        #[serde(
            rename = "amountUsdc",
            default = "defaults::faucet_request_amount_usdc"
        )]
        pub amount_usdc: f64,
        ///Solana address (base58)
        pub recipient: ::std::string::String,
    }
    ///`FaucetResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "amount": {
    ///      "type": "number"
    ///    },
    ///    "recipient": {
    ///      "type": "string"
    ///    },
    ///    "signature": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct FaucetResponse {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub amount: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub recipient: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub signature: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for FaucetResponse {
        fn default() -> Self {
            Self {
                amount: Default::default(),
                recipient: Default::default(),
                signature: Default::default(),
            }
        }
    }
    ///`GetCreditEligibilityResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "agentPubkey",
    ///    "creditLevel",
    ///    "creditScore",
    ///    "eligible",
    ///    "kyaTier",
    ///    "maxCreditUsdc",
    ///    "reason"
    ///  ],
    ///  "properties": {
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "creditLevel": {
    ///      "type": "integer"
    ///    },
    ///    "creditScore": {
    ///      "type": "integer"
    ///    },
    ///    "eligible": {
    ///      "type": "boolean"
    ///    },
    ///    "kyaTier": {
    ///      "type": "integer"
    ///    },
    ///    "maxCreditUsdc": {
    ///      "type": "integer"
    ///    },
    ///    "reason": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetCreditEligibilityResponse {
        #[serde(rename = "agentPubkey")]
        pub agent_pubkey: ::std::string::String,
        #[serde(rename = "creditLevel")]
        pub credit_level: i64,
        #[serde(rename = "creditScore")]
        pub credit_score: i64,
        pub eligible: bool,
        #[serde(rename = "kyaTier")]
        pub kya_tier: i64,
        #[serde(rename = "maxCreditUsdc")]
        pub max_credit_usdc: i64,
        pub reason: ::std::string::String,
    }
    ///`GetCreditLineResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "accruedInterest",
    ///    "agentPubkey",
    ///    "creditDrawn",
    ///    "creditLimit",
    ///    "exists",
    ///    "healthFactorBps",
    ///    "interestRateBps",
    ///    "isActive",
    ///    "isFrozen",
    ///    "lastAccrualTimestamp",
    ///    "originatedAt",
    ///    "totalInterestPaid",
    ///    "totalOwed"
    ///  ],
    ///  "properties": {
    ///    "accruedInterest": {
    ///      "type": "string"
    ///    },
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "creditDrawn": {
    ///      "type": "string"
    ///    },
    ///    "creditLimit": {
    ///      "type": "string"
    ///    },
    ///    "exists": {
    ///      "type": "boolean"
    ///    },
    ///    "healthFactorBps": {
    ///      "type": "integer"
    ///    },
    ///    "interestRateBps": {
    ///      "type": "integer"
    ///    },
    ///    "isActive": {
    ///      "type": "boolean"
    ///    },
    ///    "isFrozen": {
    ///      "type": "boolean"
    ///    },
    ///    "lastAccrualTimestamp": {
    ///      "type": "string"
    ///    },
    ///    "originatedAt": {
    ///      "type": "string"
    ///    },
    ///    "totalInterestPaid": {
    ///      "type": "string"
    ///    },
    ///    "totalOwed": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetCreditLineResponse {
        #[serde(rename = "accruedInterest")]
        pub accrued_interest: ::std::string::String,
        #[serde(rename = "agentPubkey")]
        pub agent_pubkey: ::std::string::String,
        #[serde(rename = "creditDrawn")]
        pub credit_drawn: ::std::string::String,
        #[serde(rename = "creditLimit")]
        pub credit_limit: ::std::string::String,
        pub exists: bool,
        #[serde(rename = "healthFactorBps")]
        pub health_factor_bps: i64,
        #[serde(rename = "interestRateBps")]
        pub interest_rate_bps: i64,
        #[serde(rename = "isActive")]
        pub is_active: bool,
        #[serde(rename = "isFrozen")]
        pub is_frozen: bool,
        #[serde(rename = "lastAccrualTimestamp")]
        pub last_accrual_timestamp: ::std::string::String,
        #[serde(rename = "originatedAt")]
        pub originated_at: ::std::string::String,
        #[serde(rename = "totalInterestPaid")]
        pub total_interest_paid: ::std::string::String,
        #[serde(rename = "totalOwed")]
        pub total_owed: ::std::string::String,
    }
    ///`GetKyaQuickResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "check",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "check": {
    ///      "type": "object",
    ///      "required": [
    ///        "agentType",
    ///        "creditLevel",
    ///        "isRegisteredOnKrexa",
    ///        "kyaTier",
    ///        "kyaVerified",
    ///        "limits",
    ///        "name",
    ///        "riskBand",
    ///        "score",
    ///        "timestamp",
    ///        "totalAssetsUsd",
    ///        "trustTier",
    ///        "txCount",
    ///        "wallet",
    ///        "walletAgeDays"
    ///      ],
    ///      "properties": {
    ///        "agentType": {
    ///          "type": "string"
    ///        },
    ///        "creditLevel": {
    ///          "type": "integer"
    ///        },
    ///        "isRegisteredOnKrexa": {
    ///          "type": "boolean"
    ///        },
    ///        "kyaTier": {
    ///          "type": "integer"
    ///        },
    ///        "kyaVerified": {
    ///          "type": "boolean"
    ///        },
    ///        "limits": {
    ///          "type": "object",
    ///          "required": [
    ///            "dailyLimitUsd",
    ///            "maxTransactionUsd"
    ///          ],
    ///          "properties": {
    ///            "dailyLimitUsd": {
    ///              "type": "integer"
    ///            },
    ///            "maxTransactionUsd": {
    ///              "type": "integer"
    ///            }
    ///          }
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        },
    ///        "riskBand": {
    ///          "type": "string"
    ///        },
    ///        "score": {
    ///          "type": "integer"
    ///        },
    ///        "timestamp": {
    ///          "type": "string"
    ///        },
    ///        "totalAssetsUsd": {
    ///          "type": "integer"
    ///        },
    ///        "trustTier": {
    ///          "type": "string"
    ///        },
    ///        "txCount": {
    ///          "type": "integer"
    ///        },
    ///        "wallet": {
    ///          "type": "string"
    ///        },
    ///        "walletAgeDays": {
    ///          "type": "integer"
    ///        }
    ///      }
    ///    },
    ///    "success": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetKyaQuickResponse {
        pub check: GetKyaQuickResponseCheck,
        pub success: bool,
    }
    ///`GetKyaQuickResponseCheck`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "agentType",
    ///    "creditLevel",
    ///    "isRegisteredOnKrexa",
    ///    "kyaTier",
    ///    "kyaVerified",
    ///    "limits",
    ///    "name",
    ///    "riskBand",
    ///    "score",
    ///    "timestamp",
    ///    "totalAssetsUsd",
    ///    "trustTier",
    ///    "txCount",
    ///    "wallet",
    ///    "walletAgeDays"
    ///  ],
    ///  "properties": {
    ///    "agentType": {
    ///      "type": "string"
    ///    },
    ///    "creditLevel": {
    ///      "type": "integer"
    ///    },
    ///    "isRegisteredOnKrexa": {
    ///      "type": "boolean"
    ///    },
    ///    "kyaTier": {
    ///      "type": "integer"
    ///    },
    ///    "kyaVerified": {
    ///      "type": "boolean"
    ///    },
    ///    "limits": {
    ///      "type": "object",
    ///      "required": [
    ///        "dailyLimitUsd",
    ///        "maxTransactionUsd"
    ///      ],
    ///      "properties": {
    ///        "dailyLimitUsd": {
    ///          "type": "integer"
    ///        },
    ///        "maxTransactionUsd": {
    ///          "type": "integer"
    ///        }
    ///      }
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "riskBand": {
    ///      "type": "string"
    ///    },
    ///    "score": {
    ///      "type": "integer"
    ///    },
    ///    "timestamp": {
    ///      "type": "string"
    ///    },
    ///    "totalAssetsUsd": {
    ///      "type": "integer"
    ///    },
    ///    "trustTier": {
    ///      "type": "string"
    ///    },
    ///    "txCount": {
    ///      "type": "integer"
    ///    },
    ///    "wallet": {
    ///      "type": "string"
    ///    },
    ///    "walletAgeDays": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetKyaQuickResponseCheck {
        #[serde(rename = "agentType")]
        pub agent_type: ::std::string::String,
        #[serde(rename = "creditLevel")]
        pub credit_level: i64,
        #[serde(rename = "isRegisteredOnKrexa")]
        pub is_registered_on_krexa: bool,
        #[serde(rename = "kyaTier")]
        pub kya_tier: i64,
        #[serde(rename = "kyaVerified")]
        pub kya_verified: bool,
        pub limits: GetKyaQuickResponseCheckLimits,
        pub name: ::std::string::String,
        #[serde(rename = "riskBand")]
        pub risk_band: ::std::string::String,
        pub score: i64,
        pub timestamp: ::std::string::String,
        #[serde(rename = "totalAssetsUsd")]
        pub total_assets_usd: i64,
        #[serde(rename = "trustTier")]
        pub trust_tier: ::std::string::String,
        #[serde(rename = "txCount")]
        pub tx_count: i64,
        pub wallet: ::std::string::String,
        #[serde(rename = "walletAgeDays")]
        pub wallet_age_days: i64,
    }
    ///`GetKyaQuickResponseCheckLimits`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "dailyLimitUsd",
    ///    "maxTransactionUsd"
    ///  ],
    ///  "properties": {
    ///    "dailyLimitUsd": {
    ///      "type": "integer"
    ///    },
    ///    "maxTransactionUsd": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetKyaQuickResponseCheckLimits {
        #[serde(rename = "dailyLimitUsd")]
        pub daily_limit_usd: i64,
        #[serde(rename = "maxTransactionUsd")]
        pub max_transaction_usd: i64,
    }
    ///`InviteChallengeRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "code",
    ///    "wallet"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "description": "Invite code in `KREXA-XXXX-XXXX` format.",
    ///      "type": "string"
    ///    },
    ///    "wallet": {
    ///      "description": "Agent wallet pubkey (base58) that will be activated.",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct InviteChallengeRequest {
        ///Invite code in `KREXA-XXXX-XXXX` format.
        pub code: ::std::string::String,
        ///Agent wallet pubkey (base58) that will be activated.
        pub wallet: ::std::string::String,
    }
    ///`InviteChallengeResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "challenge"
    ///  ],
    ///  "properties": {
    ///    "challenge": {
    ///      "description": "UTF-8 string to sign with the agent's Ed25519 keypair. Format\nis `\"Krexa invite redemption: <code> for <wallet>\"`.\n",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct InviteChallengeResponse {
        /**UTF-8 string to sign with the agent's Ed25519 keypair. Format
        is `"Krexa invite redemption: <code> for <wallet>"`.
        */
        pub challenge: ::std::string::String,
    }
    /**Signed KYA credential with 7 sections. Inner shape is documented
    on the platform but not fully reified here — treat as a loose
    object until the Krexa team publishes a JSON Schema.
    */
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Signed KYA credential with 7 sections. Inner shape is documented\non the platform but not fully reified here — treat as a loose\nobject until the Krexa team publishes a JSON Schema.\n",
    ///  "type": "object",
    ///  "properties": {
    ///    "attestation": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "financial": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "identity": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "permissions": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "principal": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "reputation": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "score": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct KyaCredential {
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub attestation: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub financial: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub identity: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub permissions: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub principal: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub reputation: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub score: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    }
    impl ::std::default::Default for KyaCredential {
        fn default() -> Self {
            Self {
                attestation: Default::default(),
                financial: Default::default(),
                identity: Default::default(),
                permissions: Default::default(),
                principal: Default::default(),
                reputation: Default::default(),
                score: Default::default(),
            }
        }
    }
    ///`KyaQuick`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "defaultCount": {
    ///      "type": "integer"
    ///    },
    ///    "level": {
    ///      "type": "integer"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "onTimeRate": {
    ///      "type": "number",
    ///      "format": "double"
    ///    },
    ///    "operatingDays": {
    ///      "type": "integer"
    ///    },
    ///    "score": {
    ///      "type": "integer"
    ///    },
    ///    "trustTier": {
    ///      "description": "One of unverified, identified, provisional, trusted, established, premium",
    ///      "type": "string"
    ///    },
    ///    "type": {
    ///      "description": "Agent type label",
    ///      "type": "string"
    ///    },
    ///    "verified": {
    ///      "type": "boolean"
    ///    },
    ///    "wallet": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct KyaQuick {
        #[serde(
            rename = "defaultCount",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub default_count: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub level: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "onTimeRate",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub on_time_rate: ::std::option::Option<f64>,
        #[serde(
            rename = "operatingDays",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub operating_days: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub score: ::std::option::Option<i64>,
        ///One of unverified, identified, provisional, trusted, established, premium
        #[serde(
            rename = "trustTier",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub trust_tier: ::std::option::Option<::std::string::String>,
        ///Agent type label
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub verified: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wallet: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for KyaQuick {
        fn default() -> Self {
            Self {
                default_count: Default::default(),
                level: Default::default(),
                name: Default::default(),
                on_time_rate: Default::default(),
                operating_days: Default::default(),
                score: Default::default(),
                trust_tier: Default::default(),
                type_: Default::default(),
                verified: Default::default(),
                wallet: Default::default(),
            }
        }
    }
    ///`KyaSearchHit`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "score": {
    ///      "type": "integer"
    ///    },
    ///    "trustTier": {
    ///      "type": "string"
    ///    },
    ///    "wallet": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct KyaSearchHit {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub score: ::std::option::Option<i64>,
        #[serde(
            rename = "trustTier",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub trust_tier: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub wallet: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for KyaSearchHit {
        fn default() -> Self {
            Self {
                name: Default::default(),
                score: Default::default(),
                trust_tier: Default::default(),
                wallet: Default::default(),
            }
        }
    }
    ///`KyaSearchResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "results": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/KyaSearchHit"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct KyaSearchResponse {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub results: ::std::vec::Vec<KyaSearchHit>,
    }
    impl ::std::default::Default for KyaSearchResponse {
        fn default() -> Self {
            Self {
                results: Default::default(),
            }
        }
    }
    ///`KyaVerifyRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "credential"
    ///  ],
    ///  "properties": {
    ///    "credential": {
    ///      "$ref": "#/components/schemas/KyaCredential"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct KyaVerifyRequest {
        pub credential: KyaCredential,
    }
    ///`KyaVerifyResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "errors": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "expired": {
    ///      "type": "boolean"
    ///    },
    ///    "issuerTrusted": {
    ///      "type": "boolean"
    ///    },
    ///    "onChainMatch": {
    ///      "type": "boolean"
    ///    },
    ///    "valid": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct KyaVerifyResponse {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub errors: ::std::vec::Vec<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub expired: ::std::option::Option<bool>,
        #[serde(
            rename = "issuerTrusted",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub issuer_trusted: ::std::option::Option<bool>,
        #[serde(
            rename = "onChainMatch",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub on_chain_match: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub valid: ::std::option::Option<bool>,
    }
    impl ::std::default::Default for KyaVerifyResponse {
        fn default() -> Self {
            Self {
                errors: Default::default(),
                expired: Default::default(),
                issuer_trusted: Default::default(),
                on_chain_match: Default::default(),
                valid: Default::default(),
            }
        }
    }
    ///`Pagination`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "limit": {
    ///      "type": "integer"
    ///    },
    ///    "page": {
    ///      "type": "integer"
    ///    },
    ///    "total": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Pagination {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub limit: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub page: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total: ::std::option::Option<i64>,
    }
    impl ::std::default::Default for Pagination {
        fn default() -> Self {
            Self {
                limit: Default::default(),
                page: Default::default(),
                total: Default::default(),
            }
        }
    }
    ///`PayshAutoCreditDraw`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": [
    ///    "object",
    ///    "null"
    ///  ],
    ///  "properties": {
    ///    "amountBaseUnits": {
    ///      "type": "string"
    ///    },
    ///    "amountUsdc": {
    ///      "type": "number"
    ///    },
    ///    "endpoint": {
    ///      "type": "string"
    ///    },
    ///    "note": {
    ///      "type": "string"
    ///    },
    ///    "params": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct PayshAutoCreditDraw(pub ::std::option::Option<PayshAutoCreditDrawInner>);
    impl ::std::ops::Deref for PayshAutoCreditDraw {
        type Target = ::std::option::Option<PayshAutoCreditDrawInner>;
        fn deref(&self) -> &::std::option::Option<PayshAutoCreditDrawInner> {
            &self.0
        }
    }
    impl ::std::convert::From<PayshAutoCreditDraw> for ::std::option::Option<PayshAutoCreditDrawInner> {
        fn from(value: PayshAutoCreditDraw) -> Self {
            value.0
        }
    }
    impl ::std::convert::From<::std::option::Option<PayshAutoCreditDrawInner>> for PayshAutoCreditDraw {
        fn from(value: ::std::option::Option<PayshAutoCreditDrawInner>) -> Self {
            Self(value)
        }
    }
    ///`PayshAutoCreditDrawInner`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "amountBaseUnits": {
    ///      "type": "string"
    ///    },
    ///    "amountUsdc": {
    ///      "type": "number"
    ///    },
    ///    "endpoint": {
    ///      "type": "string"
    ///    },
    ///    "note": {
    ///      "type": "string"
    ///    },
    ///    "params": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshAutoCreditDrawInner {
        #[serde(
            rename = "amountBaseUnits",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub amount_base_units: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "amountUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub amount_usdc: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub endpoint: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub note: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub params: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    }
    impl ::std::default::Default for PayshAutoCreditDrawInner {
        fn default() -> Self {
            Self {
                amount_base_units: Default::default(),
                amount_usdc: Default::default(),
                endpoint: Default::default(),
                note: Default::default(),
                params: Default::default(),
            }
        }
    }
    ///`PayshBalance`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "creditLine": {
    ///      "type": "object",
    ///      "properties": {
    ///        "available": {
    ///          "type": "boolean"
    ///        },
    ///        "limitUsdc": {
    ///          "type": "number"
    ///        },
    ///        "remainingUsdc": {
    ///          "type": "number"
    ///        },
    ///        "usedUsdc": {
    ///          "type": "number"
    ///        }
    ///      }
    ///    },
    ///    "isFrozen": {
    ///      "type": "boolean"
    ///    },
    ///    "totalSpendableUsdc": {
    ///      "type": "number"
    ///    },
    ///    "walletBalanceUsdc": {
    ///      "type": "number"
    ///    },
    ///    "walletExists": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshBalance {
        #[serde(
            rename = "creditLine",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub credit_line: ::std::option::Option<PayshBalanceCreditLine>,
        #[serde(
            rename = "isFrozen",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub is_frozen: ::std::option::Option<bool>,
        #[serde(
            rename = "totalSpendableUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub total_spendable_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "walletBalanceUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub wallet_balance_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "walletExists",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub wallet_exists: ::std::option::Option<bool>,
    }
    impl ::std::default::Default for PayshBalance {
        fn default() -> Self {
            Self {
                credit_line: Default::default(),
                is_frozen: Default::default(),
                total_spendable_usdc: Default::default(),
                wallet_balance_usdc: Default::default(),
                wallet_exists: Default::default(),
            }
        }
    }
    ///`PayshBalanceCreditLine`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "available": {
    ///      "type": "boolean"
    ///    },
    ///    "limitUsdc": {
    ///      "type": "number"
    ///    },
    ///    "remainingUsdc": {
    ///      "type": "number"
    ///    },
    ///    "usedUsdc": {
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshBalanceCreditLine {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub available: ::std::option::Option<bool>,
        #[serde(
            rename = "limitUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub limit_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "remainingUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub remaining_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "usedUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub used_usdc: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for PayshBalanceCreditLine {
        fn default() -> Self {
            Self {
                available: Default::default(),
                limit_usdc: Default::default(),
                remaining_usdc: Default::default(),
                used_usdc: Default::default(),
            }
        }
    }
    ///`PayshBalanceResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "agentPubkey",
    ///    "creditLine",
    ///    "isFrozen",
    ///    "isLiquidating",
    ///    "pusdBalanceBaseUnits",
    ///    "pusdBalanceUsdc",
    ///    "supportedTokens",
    ///    "totalSpendableUsdc",
    ///    "totalSpendableWithPusd",
    ///    "walletBalanceBaseUnits",
    ///    "walletBalanceUsdc",
    ///    "walletExists"
    ///  ],
    ///  "properties": {
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "creditLine": {
    ///      "type": "object",
    ///      "required": [
    ///        "availableCredit",
    ///        "availableCreditUsdc",
    ///        "creditDrawn",
    ///        "creditLimit",
    ///        "exists",
    ///        "healthFactorBps",
    ///        "isActive"
    ///      ],
    ///      "properties": {
    ///        "availableCredit": {
    ///          "type": "string"
    ///        },
    ///        "availableCreditUsdc": {
    ///          "type": "integer"
    ///        },
    ///        "creditDrawn": {
    ///          "type": "string"
    ///        },
    ///        "creditLimit": {
    ///          "type": "string"
    ///        },
    ///        "exists": {
    ///          "type": "boolean"
    ///        },
    ///        "healthFactorBps": {
    ///          "type": "integer"
    ///        },
    ///        "isActive": {
    ///          "type": "boolean"
    ///        }
    ///      }
    ///    },
    ///    "isFrozen": {
    ///      "type": "boolean"
    ///    },
    ///    "isLiquidating": {
    ///      "type": "boolean"
    ///    },
    ///    "pusdBalanceBaseUnits": {
    ///      "type": "string"
    ///    },
    ///    "pusdBalanceUsdc": {
    ///      "type": "integer"
    ///    },
    ///    "supportedTokens": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "totalSpendableUsdc": {
    ///      "type": "integer"
    ///    },
    ///    "totalSpendableWithPusd": {
    ///      "type": "integer"
    ///    },
    ///    "walletBalanceBaseUnits": {
    ///      "type": "string"
    ///    },
    ///    "walletBalanceUsdc": {
    ///      "type": "integer"
    ///    },
    ///    "walletExists": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshBalanceResponse {
        #[serde(rename = "agentPubkey")]
        pub agent_pubkey: ::std::string::String,
        #[serde(rename = "creditLine")]
        pub credit_line: PayshBalanceResponseCreditLine,
        #[serde(rename = "isFrozen")]
        pub is_frozen: bool,
        #[serde(rename = "isLiquidating")]
        pub is_liquidating: bool,
        #[serde(rename = "pusdBalanceBaseUnits")]
        pub pusd_balance_base_units: ::std::string::String,
        #[serde(rename = "pusdBalanceUsdc")]
        pub pusd_balance_usdc: i64,
        #[serde(rename = "supportedTokens")]
        pub supported_tokens: ::std::vec::Vec<::std::string::String>,
        #[serde(rename = "totalSpendableUsdc")]
        pub total_spendable_usdc: i64,
        #[serde(rename = "totalSpendableWithPusd")]
        pub total_spendable_with_pusd: i64,
        #[serde(rename = "walletBalanceBaseUnits")]
        pub wallet_balance_base_units: ::std::string::String,
        #[serde(rename = "walletBalanceUsdc")]
        pub wallet_balance_usdc: i64,
        #[serde(rename = "walletExists")]
        pub wallet_exists: bool,
    }
    ///`PayshBalanceResponseCreditLine`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "availableCredit",
    ///    "availableCreditUsdc",
    ///    "creditDrawn",
    ///    "creditLimit",
    ///    "exists",
    ///    "healthFactorBps",
    ///    "isActive"
    ///  ],
    ///  "properties": {
    ///    "availableCredit": {
    ///      "type": "string"
    ///    },
    ///    "availableCreditUsdc": {
    ///      "type": "integer"
    ///    },
    ///    "creditDrawn": {
    ///      "type": "string"
    ///    },
    ///    "creditLimit": {
    ///      "type": "string"
    ///    },
    ///    "exists": {
    ///      "type": "boolean"
    ///    },
    ///    "healthFactorBps": {
    ///      "type": "integer"
    ///    },
    ///    "isActive": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshBalanceResponseCreditLine {
        #[serde(rename = "availableCredit")]
        pub available_credit: ::std::string::String,
        #[serde(rename = "availableCreditUsdc")]
        pub available_credit_usdc: i64,
        #[serde(rename = "creditDrawn")]
        pub credit_drawn: ::std::string::String,
        #[serde(rename = "creditLimit")]
        pub credit_limit: ::std::string::String,
        pub exists: bool,
        #[serde(rename = "healthFactorBps")]
        pub health_factor_bps: i64,
        #[serde(rename = "isActive")]
        pub is_active: bool,
    }
    ///`PayshBudget`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "dailyLimitUsdc": {
    ///      "type": "number"
    ///    },
    ///    "isPaused": {
    ///      "type": "boolean"
    ///    },
    ///    "monthlyCapUsdc": {
    ///      "type": "number"
    ///    },
    ///    "perCallMaxUsdc": {
    ///      "type": "number"
    ///    },
    ///    "usage": {
    ///      "type": "object",
    ///      "properties": {
    ///        "dailyPct": {
    ///          "type": "number"
    ///        },
    ///        "monthlyPct": {
    ///          "type": "number"
    ///        },
    ///        "thisMonth": {
    ///          "type": "number"
    ///        },
    ///        "today": {
    ///          "type": "number"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshBudget {
        #[serde(
            rename = "dailyLimitUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub daily_limit_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "isPaused",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub is_paused: ::std::option::Option<bool>,
        #[serde(
            rename = "monthlyCapUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub monthly_cap_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "perCallMaxUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub per_call_max_usdc: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub usage: ::std::option::Option<PayshBudgetUsage>,
    }
    impl ::std::default::Default for PayshBudget {
        fn default() -> Self {
            Self {
                daily_limit_usdc: Default::default(),
                is_paused: Default::default(),
                monthly_cap_usdc: Default::default(),
                per_call_max_usdc: Default::default(),
                usage: Default::default(),
            }
        }
    }
    ///`PayshBudgetUpdate`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "alertAtPct": {
    ///      "type": "number"
    ///    },
    ///    "dailyLimitUsdc": {
    ///      "type": "number"
    ///    },
    ///    "isPaused": {
    ///      "type": "boolean"
    ///    },
    ///    "monthlyCapUsdc": {
    ///      "type": "number"
    ///    },
    ///    "perCallMaxUsdc": {
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshBudgetUpdate {
        #[serde(
            rename = "alertAtPct",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub alert_at_pct: ::std::option::Option<f64>,
        #[serde(
            rename = "dailyLimitUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub daily_limit_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "isPaused",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub is_paused: ::std::option::Option<bool>,
        #[serde(
            rename = "monthlyCapUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub monthly_cap_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "perCallMaxUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub per_call_max_usdc: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for PayshBudgetUpdate {
        fn default() -> Self {
            Self {
                alert_at_pct: Default::default(),
                daily_limit_usdc: Default::default(),
                is_paused: Default::default(),
                monthly_cap_usdc: Default::default(),
                per_call_max_usdc: Default::default(),
            }
        }
    }
    ///`PayshBudgetUsage`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "dailyPct": {
    ///      "type": "number"
    ///    },
    ///    "monthlyPct": {
    ///      "type": "number"
    ///    },
    ///    "thisMonth": {
    ///      "type": "number"
    ///    },
    ///    "today": {
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshBudgetUsage {
        #[serde(
            rename = "dailyPct",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub daily_pct: ::std::option::Option<f64>,
        #[serde(
            rename = "monthlyPct",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub monthly_pct: ::std::option::Option<f64>,
        #[serde(
            rename = "thisMonth",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub this_month: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub today: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for PayshBudgetUsage {
        fn default() -> Self {
            Self {
                daily_pct: Default::default(),
                monthly_pct: Default::default(),
                this_month: Default::default(),
                today: Default::default(),
            }
        }
    }
    ///`PayshCallRecord`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "amountUsdc": {
    ///      "type": "number"
    ///    },
    ///    "apiDomain": {
    ///      "type": "string"
    ///    },
    ///    "calledAt": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "creditUsed": {
    ///      "type": "boolean"
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "method": {
    ///      "type": "string"
    ///    },
    ///    "status": {
    ///      "type": "string"
    ///    },
    ///    "targetUrl": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshCallRecord {
        #[serde(
            rename = "amountUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub amount_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "apiDomain",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub api_domain: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "calledAt",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub called_at: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
        #[serde(
            rename = "creditUsed",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub credit_used: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub method: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "targetUrl",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub target_url: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PayshCallRecord {
        fn default() -> Self {
            Self {
                amount_usdc: Default::default(),
                api_domain: Default::default(),
                called_at: Default::default(),
                credit_used: Default::default(),
                id: Default::default(),
                method: Default::default(),
                status: Default::default(),
                target_url: Default::default(),
            }
        }
    }
    ///`PayshCallRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "ownerAddress",
    ///    "targetUrl"
    ///  ],
    ///  "properties": {
    ///    "body": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "headers": {
    ///      "description": "Auth/cookie headers are stripped before proxying",
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "maxPaymentUsdc": {
    ///      "type": "number"
    ///    },
    ///    "method": {
    ///      "description": "GET, POST, PUT, DELETE (default GET)",
    ///      "default": "GET",
    ///      "type": "string"
    ///    },
    ///    "ownerAddress": {
    ///      "type": "string"
    ///    },
    ///    "targetUrl": {
    ///      "type": "string",
    ///      "format": "uri"
    ///    },
    ///    "useCredit": {
    ///      "default": false,
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshCallRequest {
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub body: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        ///Auth/cookie headers are stripped before proxying
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub headers: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
        #[serde(
            rename = "maxPaymentUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub max_payment_usdc: ::std::option::Option<f64>,
        ///GET, POST, PUT, DELETE (default GET)
        #[serde(default = "defaults::paysh_call_request_method")]
        pub method: ::std::string::String,
        #[serde(rename = "ownerAddress")]
        pub owner_address: ::std::string::String,
        #[serde(rename = "targetUrl")]
        pub target_url: ::std::string::String,
        #[serde(rename = "useCredit", default)]
        pub use_credit: bool,
    }
    ///`PayshCallResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "apiPriceUsdc": {
    ///      "type": "number"
    ///    },
    ///    "autoCreditDraw": {
    ///      "$ref": "#/components/schemas/PayshAutoCreditDraw"
    ///    },
    ///    "funded": {
    ///      "type": "boolean"
    ///    },
    ///    "paymentRequired": {
    ///      "type": "boolean"
    ///    },
    ///    "retryInstructions": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "transaction": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshCallResponse {
        #[serde(
            rename = "agentPubkey",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub agent_pubkey: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "apiPriceUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub api_price_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "autoCreditDraw",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub auto_credit_draw: ::std::option::Option<PayshAutoCreditDraw>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub funded: ::std::option::Option<bool>,
        #[serde(
            rename = "paymentRequired",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub payment_required: ::std::option::Option<bool>,
        #[serde(
            rename = "retryInstructions",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub retry_instructions: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub transaction: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PayshCallResponse {
        fn default() -> Self {
            Self {
                agent_pubkey: Default::default(),
                api_price_usdc: Default::default(),
                auto_credit_draw: Default::default(),
                funded: Default::default(),
                payment_required: Default::default(),
                retry_instructions: Default::default(),
                transaction: Default::default(),
            }
        }
    }
    ///`PayshCatalog`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "apis": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/PayshCatalogEntry"
    ///      }
    ///    },
    ///    "count": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshCatalog {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub apis: ::std::vec::Vec<PayshCatalogEntry>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub count: ::std::option::Option<i64>,
    }
    impl ::std::default::Default for PayshCatalog {
        fn default() -> Self {
            Self {
                apis: Default::default(),
                count: Default::default(),
            }
        }
    }
    ///`PayshCatalogEntry`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "category": {
    ///      "type": "string"
    ///    },
    ///    "description": {
    ///      "type": "string"
    ///    },
    ///    "estimatedCostUsdc": {
    ///      "type": "string"
    ///    },
    ///    "exampleUrl": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "provider": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshCatalogEntry {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub category: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub description: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "estimatedCostUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub estimated_cost_usdc: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "exampleUrl",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub example_url: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub provider: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PayshCatalogEntry {
        fn default() -> Self {
            Self {
                category: Default::default(),
                description: Default::default(),
                estimated_cost_usdc: Default::default(),
                example_url: Default::default(),
                name: Default::default(),
                provider: Default::default(),
            }
        }
    }
    ///`PayshConfirmRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "txSignature"
    ///  ],
    ///  "properties": {
    ///    "responseCode": {
    ///      "type": "integer"
    ///    },
    ///    "txSignature": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshConfirmRequest {
        #[serde(
            rename = "responseCode",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub response_code: ::std::option::Option<i64>,
        #[serde(rename = "txSignature")]
        pub tx_signature: ::std::string::String,
    }
    ///`PayshConfirmResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "confirmed": {
    ///      "type": "boolean"
    ///    },
    ///    "txSignature": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshConfirmResponse {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub confirmed: ::std::option::Option<bool>,
        #[serde(
            rename = "txSignature",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub tx_signature: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PayshConfirmResponse {
        fn default() -> Self {
            Self {
                confirmed: Default::default(),
                tx_signature: Default::default(),
            }
        }
    }
    ///`PayshDiscoverRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "targetUrl"
    ///  ],
    ///  "properties": {
    ///    "method": {
    ///      "default": "GET",
    ///      "type": "string"
    ///    },
    ///    "targetUrl": {
    ///      "type": "string",
    ///      "format": "uri"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshDiscoverRequest {
        #[serde(default = "defaults::paysh_discover_request_method")]
        pub method: ::std::string::String,
        #[serde(rename = "targetUrl")]
        pub target_url: ::std::string::String,
    }
    ///`PayshDiscoverResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "affordability": {
    ///      "type": "object",
    ///      "properties": {
    ///        "canAfford": {
    ///          "type": "boolean"
    ///        },
    ///        "needsCreditDraw": {
    ///          "type": "boolean"
    ///        }
    ///      }
    ///    },
    ///    "pricing": {
    ///      "$ref": "#/components/schemas/PayshPricing"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshDiscoverResponse {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub affordability: ::std::option::Option<PayshDiscoverResponseAffordability>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pricing: ::std::option::Option<PayshPricing>,
    }
    impl ::std::default::Default for PayshDiscoverResponse {
        fn default() -> Self {
            Self {
                affordability: Default::default(),
                pricing: Default::default(),
            }
        }
    }
    ///`PayshDiscoverResponseAffordability`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "canAfford": {
    ///      "type": "boolean"
    ///    },
    ///    "needsCreditDraw": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshDiscoverResponseAffordability {
        #[serde(
            rename = "canAfford",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub can_afford: ::std::option::Option<bool>,
        #[serde(
            rename = "needsCreditDraw",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub needs_credit_draw: ::std::option::Option<bool>,
    }
    impl ::std::default::Default for PayshDiscoverResponseAffordability {
        fn default() -> Self {
            Self {
                can_afford: Default::default(),
                needs_credit_draw: Default::default(),
            }
        }
    }
    ///`PayshEstimate`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "api": {
    ///      "$ref": "#/components/schemas/PayshCatalogEntry"
    ///    },
    ///    "estimatedCostUsdc": {
    ///      "description": "Human-readable range like \"0.001-0.05\"",
    ///      "type": "string"
    ///    },
    ///    "matched": {
    ///      "type": "boolean"
    ///    },
    ///    "note": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshEstimate {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub api: ::std::option::Option<PayshCatalogEntry>,
        ///Human-readable range like "0.001-0.05"
        #[serde(
            rename = "estimatedCostUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub estimated_cost_usdc: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub matched: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub note: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PayshEstimate {
        fn default() -> Self {
            Self {
                api: Default::default(),
                estimated_cost_usdc: Default::default(),
                matched: Default::default(),
                note: Default::default(),
            }
        }
    }
    ///`PayshHistory`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "calls": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/PayshCallRecord"
    ///      }
    ///    },
    ///    "pagination": {
    ///      "$ref": "#/components/schemas/Pagination"
    ///    },
    ///    "summary": {
    ///      "$ref": "#/components/schemas/PayshHistorySummary"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshHistory {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub calls: ::std::vec::Vec<PayshCallRecord>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pagination: ::std::option::Option<Pagination>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub summary: ::std::option::Option<PayshHistorySummary>,
    }
    impl ::std::default::Default for PayshHistory {
        fn default() -> Self {
            Self {
                calls: Default::default(),
                pagination: Default::default(),
                summary: Default::default(),
            }
        }
    }
    ///`PayshHistorySummary`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "byDomain": {
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "type": "number"
    ///      }
    ///    },
    ///    "monthSpent": {
    ///      "type": "number"
    ///    },
    ///    "todaySpent": {
    ///      "type": "number"
    ///    },
    ///    "totalCalls": {
    ///      "type": "integer"
    ///    },
    ///    "totalSpent": {
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshHistorySummary {
        #[serde(
            rename = "byDomain",
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub by_domain: ::std::collections::HashMap<::std::string::String, f64>,
        #[serde(
            rename = "monthSpent",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub month_spent: ::std::option::Option<f64>,
        #[serde(
            rename = "todaySpent",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub today_spent: ::std::option::Option<f64>,
        #[serde(
            rename = "totalCalls",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub total_calls: ::std::option::Option<i64>,
        #[serde(
            rename = "totalSpent",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub total_spent: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for PayshHistorySummary {
        fn default() -> Self {
            Self {
                by_domain: Default::default(),
                month_spent: Default::default(),
                today_spent: Default::default(),
                total_calls: Default::default(),
                total_spent: Default::default(),
            }
        }
    }
    ///`PayshOnboardRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "agentPubkey",
    ///    "ownerPubkey"
    ///  ],
    ///  "properties": {
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "dailySpendLimitUsdc": {
    ///      "default": 500,
    ///      "type": "number"
    ///    },
    ///    "name": {
    ///      "default": "krexa-agent",
    ///      "type": "string",
    ///      "maxLength": 32
    ///    },
    ///    "ownerPubkey": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshOnboardRequest {
        #[serde(rename = "agentPubkey")]
        pub agent_pubkey: ::std::string::String,
        #[serde(
            rename = "dailySpendLimitUsdc",
            default = "defaults::paysh_onboard_request_daily_spend_limit_usdc"
        )]
        pub daily_spend_limit_usdc: f64,
        #[serde(default = "defaults::paysh_onboard_request_name")]
        pub name: PayshOnboardRequestName,
        #[serde(rename = "ownerPubkey")]
        pub owner_pubkey: ::std::string::String,
    }
    ///`PayshOnboardRequestName`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "krexa-agent",
    ///  "type": "string",
    ///  "maxLength": 32
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Serialize, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[serde(transparent)]
    pub struct PayshOnboardRequestName(::std::string::String);
    impl ::std::ops::Deref for PayshOnboardRequestName {
        type Target = ::std::string::String;
        fn deref(&self) -> &::std::string::String {
            &self.0
        }
    }
    impl ::std::convert::From<PayshOnboardRequestName> for ::std::string::String {
        fn from(value: PayshOnboardRequestName) -> Self {
            value.0
        }
    }
    impl ::std::default::Default for PayshOnboardRequestName {
        fn default() -> Self {
            PayshOnboardRequestName("krexa-agent".to_string())
        }
    }
    impl ::std::str::FromStr for PayshOnboardRequestName {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            if value.chars().count() > 32usize {
                return Err("longer than 32 characters".into());
            }
            Ok(Self(value.to_string()))
        }
    }
    impl ::std::convert::TryFrom<&str> for PayshOnboardRequestName {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for PayshOnboardRequestName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for PayshOnboardRequestName {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl<'de> ::serde::Deserialize<'de> for PayshOnboardRequestName {
        fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            ::std::string::String::deserialize(deserializer)?
                .parse()
                .map_err(|e: self::error::ConversionError| {
                    <D::Error as ::serde::de::Error>::custom(e.to_string())
                })
        }
    }
    ///`PayshOnboardResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "apiKey": {
    ///      "type": "string"
    ///    },
    ///    "budget": {
    ///      "type": "object",
    ///      "properties": {
    ///        "dailyLimitUsdc": {
    ///          "type": "number"
    ///        },
    ///        "perCallMaxUsdc": {
    ///          "type": "number"
    ///        }
    ///      }
    ///    },
    ///    "creditRequest": {
    ///      "type": "object",
    ///      "properties": {
    ///        "amount": {
    ///          "type": "string"
    ///        },
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "level": {
    ///          "type": "integer"
    ///        },
    ///        "status": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "encoding": {
    ///      "examples": [
    ///        "base64"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "ownerPubkey": {
    ///      "type": "string"
    ///    },
    ///    "settlement": {
    ///      "type": "object",
    ///      "properties": {
    ///        "included": {
    ///          "type": "boolean"
    ///        },
    ///        "splitBps": {
    ///          "type": "integer"
    ///        }
    ///      }
    ///    },
    ///    "status": {
    ///      "description": "`ready` if a new wallet was created; `exists` if it already existed",
    ///      "type": "string"
    ///    },
    ///    "steps": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "transaction": {
    ///      "description": "Base64-encoded unsigned transaction, or null if status is `exists`",
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshOnboardResponse {
        #[serde(
            rename = "agentPubkey",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub agent_pubkey: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "apiKey",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub api_key: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub budget: ::std::option::Option<PayshOnboardResponseBudget>,
        #[serde(
            rename = "creditRequest",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub credit_request: ::std::option::Option<PayshOnboardResponseCreditRequest>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub encoding: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub message: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "ownerPubkey",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub owner_pubkey: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub settlement: ::std::option::Option<PayshOnboardResponseSettlement>,
        ///`ready` if a new wallet was created; `exists` if it already existed
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub steps: ::std::vec::Vec<::std::string::String>,
        ///Base64-encoded unsigned transaction, or null if status is `exists`
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub transaction: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PayshOnboardResponse {
        fn default() -> Self {
            Self {
                agent_pubkey: Default::default(),
                api_key: Default::default(),
                budget: Default::default(),
                credit_request: Default::default(),
                encoding: Default::default(),
                message: Default::default(),
                owner_pubkey: Default::default(),
                settlement: Default::default(),
                status: Default::default(),
                steps: Default::default(),
                transaction: Default::default(),
            }
        }
    }
    ///`PayshOnboardResponseBudget`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "dailyLimitUsdc": {
    ///      "type": "number"
    ///    },
    ///    "perCallMaxUsdc": {
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshOnboardResponseBudget {
        #[serde(
            rename = "dailyLimitUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub daily_limit_usdc: ::std::option::Option<f64>,
        #[serde(
            rename = "perCallMaxUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub per_call_max_usdc: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for PayshOnboardResponseBudget {
        fn default() -> Self {
            Self {
                daily_limit_usdc: Default::default(),
                per_call_max_usdc: Default::default(),
            }
        }
    }
    ///`PayshOnboardResponseCreditRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "amount": {
    ///      "type": "string"
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "level": {
    ///      "type": "integer"
    ///    },
    ///    "status": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshOnboardResponseCreditRequest {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub amount: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub level: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PayshOnboardResponseCreditRequest {
        fn default() -> Self {
            Self {
                amount: Default::default(),
                id: Default::default(),
                level: Default::default(),
                status: Default::default(),
            }
        }
    }
    ///`PayshOnboardResponseSettlement`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "included": {
    ///      "type": "boolean"
    ///    },
    ///    "splitBps": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshOnboardResponseSettlement {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub included: ::std::option::Option<bool>,
        #[serde(
            rename = "splitBps",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub split_bps: ::std::option::Option<i64>,
    }
    impl ::std::default::Default for PayshOnboardResponseSettlement {
        fn default() -> Self {
            Self {
                included: Default::default(),
                split_bps: Default::default(),
            }
        }
    }
    ///`PayshPricing`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "amount": {
    ///      "description": "Base-unit amount as a string of digits",
    ///      "type": "string"
    ///    },
    ///    "asset": {
    ///      "type": "string"
    ///    },
    ///    "network": {
    ///      "type": "string"
    ///    },
    ///    "payTo": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshPricing {
        ///Base-unit amount as a string of digits
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub amount: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub asset: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub network: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "payTo",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub pay_to: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PayshPricing {
        fn default() -> Self {
            Self {
                amount: Default::default(),
                asset: Default::default(),
                network: Default::default(),
                pay_to: Default::default(),
            }
        }
    }
    ///`PayshProxyQuoteRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "provider",
    ///    "route"
    ///  ],
    ///  "properties": {
    ///    "provider": {
    ///      "description": "Provider id (e.g. `openai`, `anthropic`).",
    ///      "type": "string"
    ///    },
    ///    "route": {
    ///      "description": "Upstream route to quote against, including leading slash\n(e.g. `/v1/chat/completions`).\n",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshProxyQuoteRequest {
        ///Provider id (e.g. `openai`, `anthropic`).
        pub provider: ::std::string::String,
        /**Upstream route to quote against, including leading slash
        (e.g. `/v1/chat/completions`).
        */
        pub route: ::std::string::String,
    }
    ///`PayshProxyQuoteResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "payTo": {
    ///      "description": "Gateway wallet pubkey (base58) the agent must pay to.",
    ///      "type": "string"
    ///    },
    ///    "priceBaseUnits": {
    ///      "description": "Quoted price in USDC base units (6 decimals) as a string of\ndigits — matches the format `borrow_usdc.amount` expects.\n",
    ///      "type": "string"
    ///    },
    ///    "priceUsdc": {
    ///      "description": "Quoted price in human-readable USDC.",
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshProxyQuoteResponse {
        ///Gateway wallet pubkey (base58) the agent must pay to.
        #[serde(
            rename = "payTo",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub pay_to: ::std::option::Option<::std::string::String>,
        /**Quoted price in USDC base units (6 decimals) as a string of
        digits — matches the format `borrow_usdc.amount` expects.
        */
        #[serde(
            rename = "priceBaseUnits",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub price_base_units: ::std::option::Option<::std::string::String>,
        ///Quoted price in human-readable USDC.
        #[serde(
            rename = "priceUsdc",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub price_usdc: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for PayshProxyQuoteResponse {
        fn default() -> Self {
            Self {
                pay_to: Default::default(),
                price_base_units: Default::default(),
                price_usdc: Default::default(),
            }
        }
    }
    ///`PayshTier`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "creditScore": {
    ///      "type": "integer"
    ///    },
    ///    "currentUsage": {
    ///      "type": "object",
    ///      "properties": {
    ///        "dailySpent": {
    ///          "type": "number"
    ///        },
    ///        "monthlySpent": {
    ///          "type": "number"
    ///        }
    ///      }
    ///    },
    ///    "tier": {
    ///      "description": "e.g. Builder, Pro",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshTier {
        #[serde(
            rename = "agentPubkey",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub agent_pubkey: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "creditScore",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub credit_score: ::std::option::Option<i64>,
        #[serde(
            rename = "currentUsage",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub current_usage: ::std::option::Option<PayshTierCurrentUsage>,
        ///e.g. Builder, Pro
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tier: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PayshTier {
        fn default() -> Self {
            Self {
                agent_pubkey: Default::default(),
                credit_score: Default::default(),
                current_usage: Default::default(),
                tier: Default::default(),
            }
        }
    }
    ///`PayshTierCurrentUsage`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "dailySpent": {
    ///      "type": "number"
    ///    },
    ///    "monthlySpent": {
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PayshTierCurrentUsage {
        #[serde(
            rename = "dailySpent",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub daily_spent: ::std::option::Option<f64>,
        #[serde(
            rename = "monthlySpent",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub monthly_spent: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for PayshTierCurrentUsage {
        fn default() -> Self {
            Self {
                daily_spent: Default::default(),
                monthly_spent: Default::default(),
            }
        }
    }
    ///`PreviewBreakdown`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "base": {
    ///      "type": "integer"
    ///    },
    ///    "transactionActivity": {
    ///      "type": "integer"
    ///    },
    ///    "walletAge": {
    ///      "type": "integer"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PreviewBreakdown {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base: ::std::option::Option<i64>,
        #[serde(
            rename = "transactionActivity",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub transaction_activity: ::std::option::Option<i64>,
        #[serde(
            rename = "walletAge",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub wallet_age: ::std::option::Option<i64>,
    }
    impl ::std::default::Default for PreviewBreakdown {
        fn default() -> Self {
            Self {
                base: Default::default(),
                transaction_activity: Default::default(),
                wallet_age: Default::default(),
            }
        }
    }
    ///`ProvisionKeyRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "signature",
    ///    "wallet"
    ///  ],
    ///  "properties": {
    ///    "signature": {
    ///      "description": "Base58-encoded Ed25519 signature over the literal message\n`Krexa API key provision for <wallet>`.\n",
    ///      "type": "string"
    ///    },
    ///    "wallet": {
    ///      "description": "Solana wallet address (base58)",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ProvisionKeyRequest {
        /**Base58-encoded Ed25519 signature over the literal message
        `Krexa API key provision for <wallet>`.
        */
        pub signature: ::std::string::String,
        ///Solana wallet address (base58)
        pub wallet: ::std::string::String,
    }
    ///`ProvisionKeyResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "key": {
    ///      "description": "`kx_`-prefixed API key — store it; not retrievable later",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ProvisionKeyResponse {
        ///`kx_`-prefixed API key — store it; not retrievable later
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub key: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ProvisionKeyResponse {
        fn default() -> Self {
            Self {
                key: Default::default(),
            }
        }
    }
    ///`RedeemInviteRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "code",
    ///    "signature",
    ///    "wallet"
    ///  ],
    ///  "properties": {
    ///    "code": {
    ///      "type": "string"
    ///    },
    ///    "signature": {
    ///      "description": "Base58-encoded Ed25519 signature over the challenge string\nfrom `/access/challenge`, signed with the agent's keypair.\n",
    ///      "type": "string"
    ///    },
    ///    "wallet": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct RedeemInviteRequest {
        pub code: ::std::string::String,
        /**Base58-encoded Ed25519 signature over the challenge string
        from `/access/challenge`, signed with the agent's keypair.
        */
        pub signature: ::std::string::String,
        pub wallet: ::std::string::String,
    }
    ///`RedeemInviteResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "activated": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct RedeemInviteResponse {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub activated: ::std::option::Option<bool>,
    }
    impl ::std::default::Default for RedeemInviteResponse {
        fn default() -> Self {
            Self {
                activated: Default::default(),
            }
        }
    }
    ///`RequestCreditRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "amount",
    ///    "creditLevel",
    ///    "ownerPubkey",
    ///    "ownerSignature"
    ///  ],
    ///  "properties": {
    ///    "amount": {
    ///      "description": "Requested USDC amount in 6-decimal base units, as a string of\ndigits. `\"500000000\"` means $500.\n",
    ///      "type": "string"
    ///    },
    ///    "creditLevel": {
    ///      "description": "Requested credit level (1-4). Capped by score.",
    ///      "type": "integer",
    ///      "maximum": 4.0,
    ///      "minimum": 1.0
    ///    },
    ///    "ownerPubkey": {
    ///      "description": "Owner wallet pubkey (base58). For self-custodied agents this\nequals the agent pubkey; for delegated agents it is the\ncontrolling wallet.\n",
    ///      "type": "string"
    ///    },
    ///    "ownerSignature": {
    ///      "description": "Base64-encoded Ed25519 signature of the **raw 32 bytes** of\nthe agent's pubkey, signed with the owner's keypair.\nReference: `nacl.sign.detached(agentPubkey.toBytes(),\nownerKeypair.secretKey)` → base64.\nNOTE: not a challenge string, and base64 (not base58).\n",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct RequestCreditRequest {
        /**Requested USDC amount in 6-decimal base units, as a string of
        digits. `"500000000"` means $500.
        */
        pub amount: ::std::string::String,
        ///Requested credit level (1-4). Capped by score.
        #[serde(rename = "creditLevel")]
        pub credit_level: ::std::num::NonZeroU64,
        /**Owner wallet pubkey (base58). For self-custodied agents this
        equals the agent pubkey; for delegated agents it is the
        controlling wallet.
        */
        #[serde(rename = "ownerPubkey")]
        pub owner_pubkey: ::std::string::String,
        /**Base64-encoded Ed25519 signature of the **raw 32 bytes** of
        the agent's pubkey, signed with the owner's keypair.
        Reference: `nacl.sign.detached(agentPubkey.toBytes(),
        ownerKeypair.secretKey)` → base64.
        NOTE: not a challenge string, and base64 (not base58).
        */
        #[serde(rename = "ownerSignature")]
        pub owner_signature: ::std::string::String,
    }
    /**Score plus the 5 weighted components used to derive it. Field
    names follow the developer landing-page JS sample; the components
    block is `{ repayment, profit, behavior, usage, age }`.
    */
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Score plus the 5 weighted components used to derive it. Field\nnames follow the developer landing-page JS sample; the components\nblock is `{ repayment, profit, behavior, usage, age }`.\n",
    ///  "type": "object",
    ///  "properties": {
    ///    "components": {
    ///      "type": "object",
    ///      "properties": {
    ///        "age": {
    ///          "type": "number"
    ///        },
    ///        "behavior": {
    ///          "type": "number"
    ///        },
    ///        "profit": {
    ///          "type": "number"
    ///        },
    ///        "repayment": {
    ///          "type": "number"
    ///        },
    ///        "usage": {
    ///          "type": "number"
    ///        }
    ///      }
    ///    },
    ///    "level": {
    ///      "type": "integer"
    ///    },
    ///    "score": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ScoreBreakdown {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub components: ::std::option::Option<ScoreBreakdownComponents>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub level: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub score: ::std::option::Option<i64>,
    }
    impl ::std::default::Default for ScoreBreakdown {
        fn default() -> Self {
            Self {
                components: Default::default(),
                level: Default::default(),
                score: Default::default(),
            }
        }
    }
    ///`ScoreBreakdownComponents`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "age": {
    ///      "type": "number"
    ///    },
    ///    "behavior": {
    ///      "type": "number"
    ///    },
    ///    "profit": {
    ///      "type": "number"
    ///    },
    ///    "repayment": {
    ///      "type": "number"
    ///    },
    ///    "usage": {
    ///      "type": "number"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ScoreBreakdownComponents {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub age: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub behavior: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub profit: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub repayment: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub usage: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for ScoreBreakdownComponents {
        fn default() -> Self {
            Self {
                age: Default::default(),
                behavior: Default::default(),
                profit: Default::default(),
                repayment: Default::default(),
                usage: Default::default(),
            }
        }
    }
    ///Raw component scores in basis points
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Raw component scores in basis points",
    ///  "type": "object",
    ///  "properties": {
    ///    "c1Repayment": {
    ///      "type": "integer"
    ///    },
    ///    "c2Profitability": {
    ///      "type": "integer"
    ///    },
    ///    "c3Behavioral": {
    ///      "type": "integer"
    ///    },
    ///    "c4Usage": {
    ///      "type": "integer"
    ///    },
    ///    "c5Maturity": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ScoreComponents {
        #[serde(
            rename = "c1Repayment",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub c1_repayment: ::std::option::Option<i64>,
        #[serde(
            rename = "c2Profitability",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub c2_profitability: ::std::option::Option<i64>,
        #[serde(
            rename = "c3Behavioral",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub c3_behavioral: ::std::option::Option<i64>,
        #[serde(
            rename = "c4Usage",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub c4_usage: ::std::option::Option<i64>,
        #[serde(
            rename = "c5Maturity",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub c5_maturity: ::std::option::Option<i64>,
    }
    impl ::std::default::Default for ScoreComponents {
        fn default() -> Self {
            Self {
                c1_repayment: Default::default(),
                c2_profitability: Default::default(),
                c3_behavioral: Default::default(),
                c4_usage: Default::default(),
                c5_maturity: Default::default(),
            }
        }
    }
    ///`ScorePreview`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "breakdown": {
    ///      "$ref": "#/components/schemas/PreviewBreakdown"
    ///    },
    ///    "components": {
    ///      "$ref": "#/components/schemas/ScoreComponents"
    ///    },
    ///    "network": {
    ///      "description": "e.g. mainnet-beta, devnet",
    ///      "type": "string"
    ///    },
    ///    "score": {
    ///      "type": "integer",
    ///      "maximum": 850.0,
    ///      "minimum": 200.0
    ///    },
    ///    "txCount": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ScorePreview {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub breakdown: ::std::option::Option<PreviewBreakdown>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub components: ::std::option::Option<ScoreComponents>,
        ///e.g. mainnet-beta, devnet
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub network: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub score: ::std::option::Option<i64>,
        #[serde(
            rename = "txCount",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub tx_count: ::std::option::Option<i64>,
    }
    impl ::std::default::Default for ScorePreview {
        fn default() -> Self {
            Self {
                breakdown: Default::default(),
                components: Default::default(),
                network: Default::default(),
                score: Default::default(),
                tx_count: Default::default(),
            }
        }
    }
    ///`ScoreResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "isRegistered": {
    ///      "type": "boolean"
    ///    },
    ///    "preview": {
    ///      "$ref": "#/components/schemas/ScorePreview"
    ///    },
    ///    "source": {
    ///      "description": "`onchain` or `preview`",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ScoreResponse {
        #[serde(
            rename = "agentPubkey",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub agent_pubkey: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "isRegistered",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub is_registered: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub preview: ::std::option::Option<ScorePreview>,
        ///`onchain` or `preview`
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub source: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ScoreResponse {
        fn default() -> Self {
            Self {
                agent_pubkey: Default::default(),
                is_registered: Default::default(),
                preview: Default::default(),
                source: Default::default(),
            }
        }
    }
    ///`SearchKyaAgentsResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "results",
    ///    "success"
    ///  ],
    ///  "properties": {
    ///    "results": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "additionalProperties": true
    ///      }
    ///    },
    ///    "success": {
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct SearchKyaAgentsResponse {
        pub results: ::std::vec::Vec<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        pub success: bool,
    }
    ///`SignCreditRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "agentOrOwnerPubkey",
    ///    "agentPubkey",
    ///    "amount"
    ///  ],
    ///  "properties": {
    ///    "agentOrOwnerPubkey": {
    ///      "type": "string"
    ///    },
    ///    "agentPubkey": {
    ///      "type": "string"
    ///    },
    ///    "amount": {
    ///      "description": "USDC lamports as a string of digits (e.g. `\"500000000\"` for\n$500). Sending a JSON number will fail validation.\n",
    ///      "type": "string"
    ///    },
    ///    "collateralValueUsdc": {
    ///      "default": "0",
    ///      "type": "string"
    ///    },
    ///    "creditLevel": {
    ///      "type": "integer",
    ///      "maximum": 4.0,
    ///      "minimum": 1.0
    ///    },
    ///    "rateBps": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct SignCreditRequest {
        #[serde(rename = "agentOrOwnerPubkey")]
        pub agent_or_owner_pubkey: ::std::string::String,
        #[serde(rename = "agentPubkey")]
        pub agent_pubkey: ::std::string::String,
        /**USDC lamports as a string of digits (e.g. `"500000000"` for
        $500). Sending a JSON number will fail validation.
        */
        pub amount: ::std::string::String,
        #[serde(
            rename = "collateralValueUsdc",
            default = "defaults::sign_credit_request_collateral_value_usdc"
        )]
        pub collateral_value_usdc: ::std::string::String,
        #[serde(
            rename = "creditLevel",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub credit_level: ::std::option::Option<::std::num::NonZeroU64>,
        #[serde(
            rename = "rateBps",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub rate_bps: ::std::option::Option<i64>,
    }
    ///`SignCreditResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "message": {
    ///      "type": "string"
    ///    },
    ///    "transaction": {
    ///      "description": "Base64-encoded partially-signed transaction",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct SignCreditResponse {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub message: ::std::option::Option<::std::string::String>,
        ///Base64-encoded partially-signed transaction
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub transaction: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for SignCreditResponse {
        fn default() -> Self {
            Self {
                message: Default::default(),
                transaction: Default::default(),
            }
        }
    }
    ///`VaultStats`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "totalBorrowed": {
    ///      "type": "number"
    ///    },
    ///    "tranches": {
    ///      "$ref": "#/components/schemas/VaultTranches"
    ///    },
    ///    "tvl": {
    ///      "description": "Total value locked (USDC, human-readable units)",
    ///      "type": "number"
    ///    },
    ///    "utilizationRate": {
    ///      "type": "number",
    ///      "format": "double",
    ///      "maximum": 1.0,
    ///      "minimum": 0.0
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct VaultStats {
        #[serde(
            rename = "totalBorrowed",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub total_borrowed: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tranches: ::std::option::Option<VaultTranches>,
        ///Total value locked (USDC, human-readable units)
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tvl: ::std::option::Option<f64>,
        #[serde(
            rename = "utilizationRate",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub utilization_rate: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for VaultStats {
        fn default() -> Self {
            Self {
                total_borrowed: Default::default(),
                tranches: Default::default(),
                tvl: Default::default(),
                utilization_rate: Default::default(),
            }
        }
    }
    ///`VaultTranche`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "apr": {
    ///      "type": "number"
    ///    },
    ///    "deposits": {
    ///      "type": "number"
    ///    },
    ///    "utilization": {
    ///      "type": "number",
    ///      "format": "double"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct VaultTranche {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub apr: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub deposits: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub utilization: ::std::option::Option<f64>,
    }
    impl ::std::default::Default for VaultTranche {
        fn default() -> Self {
            Self {
                apr: Default::default(),
                deposits: Default::default(),
                utilization: Default::default(),
            }
        }
    }
    ///`VaultTranches`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "junior": {
    ///      "$ref": "#/components/schemas/VaultTranche"
    ///    },
    ///    "mezzanine": {
    ///      "$ref": "#/components/schemas/VaultTranche"
    ///    },
    ///    "senior": {
    ///      "$ref": "#/components/schemas/VaultTranche"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct VaultTranches {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub junior: ::std::option::Option<VaultTranche>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub mezzanine: ::std::option::Option<VaultTranche>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub senior: ::std::option::Option<VaultTranche>,
    }
    impl ::std::default::Default for VaultTranches {
        fn default() -> Self {
            Self {
                junior: Default::default(),
                mezzanine: Default::default(),
                senior: Default::default(),
            }
        }
    }
    /// Generation of default values for serde.
    pub mod defaults {
        pub(super) fn default_i64<T, const V: i64>() -> T
        where
            T: ::std::convert::TryFrom<i64>,
            <T as ::std::convert::TryFrom<i64>>::Error: ::std::fmt::Debug,
        {
            T::try_from(V).unwrap()
        }
        pub(super) fn faucet_request_amount_usdc() -> f64 {
            100_f64
        }
        pub(super) fn paysh_call_request_method() -> ::std::string::String {
            "GET".to_string()
        }
        pub(super) fn paysh_discover_request_method() -> ::std::string::String {
            "GET".to_string()
        }
        pub(super) fn paysh_onboard_request_daily_spend_limit_usdc() -> f64 {
            500_f64
        }
        pub(super) fn paysh_onboard_request_name() -> super::PayshOnboardRequestName {
            super::PayshOnboardRequestName("krexa-agent".to_string())
        }
        pub(super) fn sign_credit_request_collateral_value_usdc() -> ::std::string::String {
            "0".to_string()
        }
    }
}
#[derive(Clone, Debug)]
/**Client for Krexa API

Krexa is a programmable credit network for AI agents on Solana. The
REST API exposes Krexit Score lookups, KYA (Know-Your-Agent) credential
issuance and verification, credit eligibility and active credit lines,
vault statistics, devnet faucet, an oracle co-signer for borrow
transactions, and the Pay.sh credit-backed API payment flow.

## Auth
Most endpoints are public. To raise the rate limit from 30 req/min to
60 req/min, callers attach an `X-API-Key: kx_...` header. Pay.sh
endpoints that mutate state (`/call`, `/discover`, `/history`,
`/budget`, `/confirm`, `/tier`) require the same header (it is
provisioned by `POST /solana/paysh/onboard`).

The API key is provisioned client-side by signing the message
`Krexa API key provision for <wallet>` with the wallet's Ed25519
keypair and posting the base58 signature to `/access/provision-key`.
The signing itself is hand-written (Solana tweetnacl); this spec only
declares the resulting header for codegen purposes. The signing shim
lives at `apps/krexa/src/auth.rs`.


Version: 1.0.0*/
pub struct Client {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest::Client,
}
impl Client {
    /// Create a new client.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new(baseurl: &str) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let client = {
            let dur = ::std::time::Duration::from_secs(15u64);
            reqwest::ClientBuilder::new()
                .connect_timeout(dur)
                .timeout(dur)
        };
        #[cfg(target_arch = "wasm32")]
        let client = reqwest::ClientBuilder::new();
        Self::new_with_client(baseurl, client.build().unwrap())
    }
    /// Construct a new client with an existing `reqwest::Client`,
    /// allowing more control over its configuration.
    ///
    /// `baseurl` is the base URL provided to the internal
    /// `reqwest::Client`, and should include a scheme and hostname,
    /// as well as port and a path stem if applicable.
    pub fn new_with_client(baseurl: &str, client: reqwest::Client) -> Self {
        Self {
            baseurl: baseurl.to_string(),
            client,
        }
    }
}
impl ClientInfo<()> for Client {
    fn api_version() -> &'static str {
        "1.0.0"
    }
    fn baseurl(&self) -> &str {
        self.baseurl.as_str()
    }
    fn client(&self) -> &reqwest::Client {
        &self.client
    }
    fn inner(&self) -> &() {
        &()
    }
}
impl ClientHooks<()> for &Client {}
#[allow(clippy::all)]
impl Client {
    /**Get full KYA credential

    Returns the signed KYA credential with 7 sections: identity,
    principal, permissions, score, reputation, financial,
    attestation.


    Sends a `GET` request to `/kya/{input}`

    */
    pub async fn get_kya_credential<'a>(
        &'a self,
        input: &'a str,
    ) -> Result<ResponseValue<types::KyaCredential>, Error<()>> {
        let url = format!("{}/kya/{}", self.baseurl, encode_path(&input.to_string()),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_kya_credential",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Quick credential check

    Minimal-field response intended for high-frequency checks.

    Sends a `GET` request to `/kya/{input}/quick`

    */
    pub async fn get_kya_quick<'a>(
        &'a self,
        input: &'a str,
    ) -> Result<ResponseValue<types::GetKyaQuickResponse>, Error<()>> {
        let url = format!(
            "{}/kya/{}/quick",
            self.baseurl,
            encode_path(&input.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_kya_quick",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Verify a KYA credential

    Verifies the credential's Ed25519 signature, expiry, and the
    issuer's trust status.


    Sends a `POST` request to `/kya/verify`

    */
    pub async fn verify_kya_credential<'a>(
        &'a self,
        body: &'a types::KyaVerifyRequest,
    ) -> Result<ResponseValue<types::KyaVerifyResponse>, Error<()>> {
        let url = format!("{}/kya/verify", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "verify_kya_credential",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Autocomplete search by wallet prefix or name

    Sends a `GET` request to `/kya/search`

    Arguments:
    - `q`: Wallet prefix or agent name fragment
    */
    pub async fn search_kya_agents<'a>(
        &'a self,
        q: &'a str,
    ) -> Result<ResponseValue<types::SearchKyaAgentsResponse>, Error<()>> {
        let url = format!("{}/kya/search", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("q", &q))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "search_kya_agents",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Get Krexit Score for an agent

    Returns the Krexit Score (200-850) and its component breakdown.
    Unregistered wallets receive a `preview` score derived from
    wallet age, transaction count, and SOL balance.


    Sends a `GET` request to `/solana/score/{agent}`

    */
    pub async fn get_score<'a>(
        &'a self,
        agent: &'a str,
    ) -> Result<
        ResponseValue<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        Error<()>,
    > {
        let url = format!(
            "{}/solana/score/{}",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_score",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Credit eligibility check

    Sends a `GET` request to `/solana/credit/{agent}/eligibility`

    */
    pub async fn get_credit_eligibility<'a>(
        &'a self,
        agent: &'a str,
    ) -> Result<ResponseValue<types::GetCreditEligibilityResponse>, Error<()>> {
        let url = format!(
            "{}/solana/credit/{}/eligibility",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_credit_eligibility",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Active credit line details

    Sends a `GET` request to `/solana/credit/{agent}/line`

    */
    pub async fn get_credit_line<'a>(
        &'a self,
        agent: &'a str,
    ) -> Result<ResponseValue<types::GetCreditLineResponse>, Error<()>> {
        let url = format!(
            "{}/solana/credit/{}/line",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_credit_line",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Submit a credit request for oracle review

    Creates a credit request that the oracle reviews before signing a
    borrow transaction. Confirmed by the Krexa team: the
    `ownerSignature` is the **base64 Ed25519 signature of the raw 32
    bytes of the agent's pubkey**, signed with the owner's keypair
    (NOT a challenge string, NOT base58).

    Once the oracle approves (synchronously for level-appropriate
    scores), call `/solana/oracle/sign-credit` to draw the line.


    Sends a `POST` request to `/solana/credit/{agent}/request`

    Arguments:
    - `agent`
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    - `body`
    */
    pub async fn request_credit<'a>(
        &'a self,
        agent: &'a str,
        x_api_key: &'a str,
        body: &'a types::RequestCreditRequest,
    ) -> Result<
        ResponseValue<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        Error<()>,
    > {
        let url = format!(
            "{}/solana/credit/{}/request",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "request_credit",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Detailed Krexit Score component breakdown

    Returns the score together with the 5 weighted components
    (repayment, profit, behavior, usage, age) and the credit level
    derived from the score.


    Sends a `GET` request to `/solana/credit/{agent}/score-breakdown`

    */
    pub async fn get_score_breakdown<'a>(
        &'a self,
        agent: &'a str,
    ) -> Result<ResponseValue<types::ScoreBreakdown>, Error<()>> {
        let url = format!(
            "{}/solana/credit/{}/score-breakdown",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_score_breakdown",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Oracle co-sign a borrow transaction

    The oracle verifies the agent's score and co-signs the borrow
    transaction. The response transaction is base64-encoded and
    partially signed; the caller must add the agent or owner
    signature before submitting to Solana.


    Sends a `POST` request to `/solana/oracle/sign-credit`

    Arguments:
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    - `body`
    */
    pub async fn sign_credit_transaction<'a>(
        &'a self,
        x_api_key: &'a str,
        body: &'a types::SignCreditRequest,
    ) -> Result<ResponseValue<types::SignCreditResponse>, Error<()>> {
        let url = format!("{}/solana/oracle/sign-credit", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "sign_credit_transaction",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Vault TVL, utilization, and tranche stats

    Sends a `GET` request to `/solana/vault/stats`

    */
    pub async fn get_vault_stats<'a>(
        &'a self,
    ) -> Result<ResponseValue<types::VaultStats>, Error<()>> {
        let url = format!("{}/solana/vault/stats", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_vault_stats",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Request test USDC (devnet only)

    Devnet-only. Disabled on mainnet-beta. Rate-limited to one call
    per 24 hours per recipient address.


    Sends a `POST` request to `/solana/faucet/usdc`

    */
    pub async fn request_faucet_usdc<'a>(
        &'a self,
        body: &'a types::FaucetRequest,
    ) -> Result<ResponseValue<types::FaucetResponse>, Error<()>> {
        let url = format!("{}/solana/faucet/usdc", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "request_faucet_usdc",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Fetch the literal challenge string for invite redemption

    Returns the exact UTF-8 string the caller must sign with their
    **agent** keypair before submitting `POST /access/redeem`. The
    string is fully determined by `code` and `wallet`
    (`"Krexa invite redemption: <code> for <wallet>"`), but the
    backend is the source of truth — always fetch it rather than
    building locally.


    Sends a `POST` request to `/access/challenge`

    */
    pub async fn get_invite_challenge<'a>(
        &'a self,
        body: &'a types::InviteChallengeRequest,
    ) -> Result<ResponseValue<types::InviteChallengeResponse>, Error<()>> {
        let url = format!("{}/access/challenge", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_invite_challenge",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Redeem an invite code with an agent-signed challenge

    Permanently binds an invite code to a Solana wallet (agent).
    One code per wallet. Once redeemed, the wallet is activated for
    all gated endpoints.

    `signature` is the base58-encoded Ed25519 signature of the
    challenge returned by `/access/challenge`, signed with the
    agent's keypair.


    Sends a `POST` request to `/access/redeem`

    */
    pub async fn redeem_invite<'a>(
        &'a self,
        body: &'a types::RedeemInviteRequest,
    ) -> Result<ResponseValue<types::RedeemInviteResponse>, Error<()>> {
        let url = format!("{}/access/redeem", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "redeem_invite",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Build the on-chain agent-deploy transaction

    Builds an unsigned-by-caller transaction that performs three
    on-chain steps as one atomic submit:
      1. `register_agent` on the registry program
      2. `update_kya` at tier 1 (pre-signed by the oracle)
      3. `create_wallet` (PDA wallet + USDC ATA)

    Idempotent: if the agent is already deployed, returns
    `{ "status": "exists" }` and `transaction: null`. Otherwise
    returns `{ "status": "ready", "transaction": "<base64 partial
    signed tx>" }` — the caller adds the agent + owner signatures
    (often the same keypair for self-custodied agents) and submits
    to Solana RPC.

    Requires the wallet to be funded with SOL (~0.01 SOL for rent on
    the three new accounts) and previously activated via
    `/access/redeem`.


    Sends a `POST` request to `/agents/deploy`

    */
    pub async fn deploy_agent<'a>(
        &'a self,
        body: &'a types::DeployAgentRequest,
    ) -> Result<ResponseValue<types::DeployAgentResponse>, Error<()>> {
        let url = format!("{}/agents/deploy", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "deploy_agent",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Provision an API key for a Solana wallet

    Self-service API key provisioning. The caller signs the message
    `Krexa API key provision for <wallet>` with their wallet
    keypair and posts the base58 signature here. The returned `kx_`
    key is then passed as `X-API-Key` on subsequent requests for
    the 60 req/min tier.


    Sends a `POST` request to `/access/provision-key`

    */
    pub async fn provision_api_key<'a>(
        &'a self,
        body: &'a types::ProvisionKeyRequest,
    ) -> Result<ResponseValue<types::ProvisionKeyResponse>, Error<()>> {
        let url = format!("{}/access/provision-key", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "provision_api_key",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**One-call agent onboarding

    Deploys an agent wallet, grants $500 L1 credit, and provisions an
    API key in a single call. Returns an unsigned transaction the
    owner must sign and submit.


    Sends a `POST` request to `/solana/paysh/onboard`

    */
    pub async fn paysh_onboard<'a>(
        &'a self,
        body: &'a types::PayshOnboardRequest,
    ) -> Result<ResponseValue<types::PayshOnboardResponse>, Error<()>> {
        let url = format!("{}/solana/paysh/onboard", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_onboard",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Full API call flow

    Probe the target URL for x402 pricing, check budget and funding,
    and build a USDC payment transaction.


    Sends a `POST` request to `/solana/paysh/{agent}/call`

    Arguments:
    - `agent`
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    - `body`
    */
    pub async fn paysh_call<'a>(
        &'a self,
        agent: &'a str,
        x_api_key: &'a str,
        body: &'a types::PayshCallRequest,
    ) -> Result<ResponseValue<types::PayshCallResponse>, Error<()>> {
        let url = format!(
            "{}/solana/paysh/{}/call",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_call",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Probe URL for x402 pricing

    Sends a `POST` request to `/solana/paysh/{agent}/discover`

    Arguments:
    - `agent`
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    - `body`
    */
    pub async fn paysh_discover<'a>(
        &'a self,
        agent: &'a str,
        x_api_key: &'a str,
        body: &'a types::PayshDiscoverRequest,
    ) -> Result<ResponseValue<types::PayshDiscoverResponse>, Error<()>> {
        let url = format!(
            "{}/solana/paysh/{}/discover",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_discover",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Wallet USDC balance and available credit

    Sends a `GET` request to `/solana/paysh/{agent}/balance`

    */
    pub async fn paysh_balance<'a>(
        &'a self,
        agent: &'a str,
    ) -> Result<ResponseValue<types::PayshBalanceResponse>, Error<()>> {
        let url = format!(
            "{}/solana/paysh/{}/balance",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_balance",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Paginated spending history with analytics

    Sends a `GET` request to `/solana/paysh/{agent}/history`

    Arguments:
    - `agent`
    - `domain`
    - `limit`
    - `page`
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    */
    pub async fn paysh_history<'a>(
        &'a self,
        agent: &'a str,
        domain: Option<&'a str>,
        limit: Option<::std::num::NonZeroU64>,
        page: Option<::std::num::NonZeroU64>,
        x_api_key: &'a str,
    ) -> Result<ResponseValue<types::PayshHistory>, Error<()>> {
        let url = format!(
            "{}/solana/paysh/{}/history",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("domain", &domain))
            .query(&progenitor_client::QueryParam::new("limit", &limit))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_history",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Get current budget limits and usage

    Sends a `GET` request to `/solana/paysh/{agent}/budget`

    Arguments:
    - `agent`
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    */
    pub async fn paysh_get_budget<'a>(
        &'a self,
        agent: &'a str,
        x_api_key: &'a str,
    ) -> Result<ResponseValue<types::PayshBudget>, Error<()>> {
        let url = format!(
            "{}/solana/paysh/{}/budget",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_get_budget",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Update spending limits

    All fields are optional; only provided fields are updated.

    Sends a `POST` request to `/solana/paysh/{agent}/budget`

    Arguments:
    - `agent`
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    - `body`
    */
    pub async fn paysh_set_budget<'a>(
        &'a self,
        agent: &'a str,
        x_api_key: &'a str,
        body: &'a types::PayshBudgetUpdate,
    ) -> Result<ResponseValue<types::PayshBudget>, Error<()>> {
        let url = format!(
            "{}/solana/paysh/{}/budget",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_set_budget",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Confirm a payment tx was submitted on-chain

    Sends a `POST` request to `/solana/paysh/{agent}/confirm`

    Arguments:
    - `agent`
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    - `body`
    */
    pub async fn paysh_confirm<'a>(
        &'a self,
        agent: &'a str,
        x_api_key: &'a str,
        body: &'a types::PayshConfirmRequest,
    ) -> Result<ResponseValue<types::PayshConfirmResponse>, Error<()>> {
        let url = format!(
            "{}/solana/paysh/{}/confirm",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_confirm",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Offline cost estimate from the curated catalog

    Sends a `GET` request to `/solana/paysh/{agent}/estimate`

    */
    pub async fn paysh_estimate<'a>(
        &'a self,
        agent: &'a str,
        url: &'a str,
    ) -> Result<ResponseValue<types::PayshEstimate>, Error<()>> {
        let _url = format!(
            "{}/solana/paysh/{}/estimate",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(_url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("url", &url))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_estimate",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Agent's score-based API access tier

    Sends a `GET` request to `/solana/paysh/{agent}/tier`

    Arguments:
    - `agent`
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    */
    pub async fn paysh_tier<'a>(
        &'a self,
        agent: &'a str,
        x_api_key: &'a str,
    ) -> Result<ResponseValue<types::PayshTier>, Error<()>> {
        let url = format!(
            "{}/solana/paysh/{}/tier",
            self.baseurl,
            encode_path(&agent.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_tier",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**List Pay.sh proxy providers with live pricing

    Returns the providers the Pay.sh proxy gateway can route to (e.g.
    OpenAI, Anthropic) along with per-route pricing and a
    `proxyAvailable: true` flag. Replaces the legacy `/paysh/catalog`
    endpoint, whose URLs were placeholders.


    Sends a `GET` request to `/solana/paysh/proxy/providers`

    */
    pub async fn paysh_proxy_list_providers<'a>(
        &'a self,
    ) -> Result<
        ResponseValue<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        Error<()>,
    > {
        let url = format!("{}/solana/paysh/proxy/providers", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_proxy_list_providers",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Quote a proxy call (price + gateway wallet)

    Quote the USDC price for a `(provider, route)` pair, plus the
    gateway wallet the agent must transfer USDC to before the proxy
    call. Three-step flow:

    1. `payshProxyQuote` → `{ priceUsdc, payTo, priceBaseUnits }`
    2. Agent sends USDC to `payTo` on Solana, captures the tx signature
    3. `payshProxyCall` with `X-PAYMENT: <tx-sig>` + `X-AGENT-PUBKEY`


    Sends a `POST` request to `/solana/paysh/proxy/quote`

    Arguments:
    - `x_api_key`: `kx_`-prefixed key from `POST /access/provision-key` or
    `POST /solana/paysh/onboard`. Required on Pay.sh authenticated
    endpoints.

    - `body`
    */
    pub async fn paysh_proxy_quote<'a>(
        &'a self,
        x_api_key: &'a str,
        body: &'a types::PayshProxyQuoteRequest,
    ) -> Result<ResponseValue<types::PayshProxyQuoteResponse>, Error<()>> {
        let url = format!("{}/solana/paysh/proxy/quote", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        header_map.append("X-API-Key", x_api_key.to_string().try_into()?);
        #[allow(unused_mut)]
        let mut request = self
            .client
            .post(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_proxy_quote",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**List Pay.sh curated APIs

    Sends a `GET` request to `/solana/paysh/catalog`

    */
    pub async fn paysh_catalog<'a>(
        &'a self,
        category: Option<&'a str>,
        provider: Option<&'a str>,
    ) -> Result<ResponseValue<types::PayshCatalog>, Error<()>> {
        let url = format!("{}/solana/paysh/catalog", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .get(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .query(&progenitor_client::QueryParam::new("category", &category))
            .query(&progenitor_client::QueryParam::new("provider", &provider))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "paysh_catalog",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => ResponseValue::from_response(response).await,
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
}
/// Items consumers will typically use such as the Client.
pub mod prelude {
    #[allow(unused_imports)]
    pub use super::Client;
}
