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
    ///`BuildDepositResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "approvals",
    ///    "kind"
    ///  ],
    ///  "properties": {
    ///    "approvals": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "request",
    ///          "type"
    ///        ],
    ///        "properties": {
    ///          "deposit": {
    ///            "description": "True iff this approval IS the deposit transaction (the final one in the array). Other entries are wallet chain switch + ERC-20 approvals.",
    ///            "type": "boolean"
    ///          },
    ///          "request": {
    ///            "type": "object",
    ///            "required": [
    ///              "method",
    ///              "params"
    ///            ],
    ///            "properties": {
    ///              "method": {
    ///                "description": "EIP-1193 method. Common values:\n  - `wallet_switchEthereumChain` (params: [{chainId}])\n  - `eth_sendTransaction`        (params: [{from,to,data,value?,chainId?}])\n",
    ///                "type": "string"
    ///              },
    ///              "params": {
    ///                "type": "array",
    ///                "items": {
    ///                  "type": "object",
    ///                  "properties": {
    ///                    "chainId": {
    ///                      "description": "Hex chain id (e.g. \"0x1\" for Ethereum). Present on wallet_switchEthereumChain AND eth_sendTransaction.",
    ///                      "type": "string"
    ///                    },
    ///                    "data": {
    ///                      "description": "0x-prefixed calldata. Only on eth_sendTransaction.",
    ///                      "type": "string"
    ///                    },
    ///                    "from": {
    ///                      "description": "Sender EVM address (0x…). Only on eth_sendTransaction.",
    ///                      "type": "string"
    ///                    },
    ///                    "gas": {
    ///                      "description": "Optional gas limit hint as a string.",
    ///                      "type": "string"
    ///                    },
    ///                    "to": {
    ///                      "description": "Target EVM address (0x…). Only on eth_sendTransaction.",
    ///                      "type": "string"
    ///                    },
    ///                    "value": {
    ///                      "description": "Wei amount as a string. Only on eth_sendTransaction.",
    ///                      "type": "string"
    ///                    }
    ///                  }
    ///                }
    ///              }
    ///            }
    ///          },
    ///          "type": {
    ///            "description": "Always `eip1193_request` today.",
    ///            "type": "string"
    ///          },
    ///          "waitForReceipt": {
    ///            "description": "True iff the host should wait for the tx receipt before continuing to the next approval.",
    ///            "type": "boolean"
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "kind": {
    ///      "description": "Deposit method dispatched by Khalani: CONTRACT_CALL, TRANSFER, etc.",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct BuildDepositResponse {
        pub approvals: ::std::vec::Vec<BuildDepositResponseApprovalsItem>,
        ///Deposit method dispatched by Khalani: CONTRACT_CALL, TRANSFER, etc.
        pub kind: ::std::string::String,
    }
    ///`BuildDepositResponseApprovalsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "request",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "deposit": {
    ///      "description": "True iff this approval IS the deposit transaction (the final one in the array). Other entries are wallet chain switch + ERC-20 approvals.",
    ///      "type": "boolean"
    ///    },
    ///    "request": {
    ///      "type": "object",
    ///      "required": [
    ///        "method",
    ///        "params"
    ///      ],
    ///      "properties": {
    ///        "method": {
    ///          "description": "EIP-1193 method. Common values:\n  - `wallet_switchEthereumChain` (params: [{chainId}])\n  - `eth_sendTransaction`        (params: [{from,to,data,value?,chainId?}])\n",
    ///          "type": "string"
    ///        },
    ///        "params": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "object",
    ///            "properties": {
    ///              "chainId": {
    ///                "description": "Hex chain id (e.g. \"0x1\" for Ethereum). Present on wallet_switchEthereumChain AND eth_sendTransaction.",
    ///                "type": "string"
    ///              },
    ///              "data": {
    ///                "description": "0x-prefixed calldata. Only on eth_sendTransaction.",
    ///                "type": "string"
    ///              },
    ///              "from": {
    ///                "description": "Sender EVM address (0x…). Only on eth_sendTransaction.",
    ///                "type": "string"
    ///              },
    ///              "gas": {
    ///                "description": "Optional gas limit hint as a string.",
    ///                "type": "string"
    ///              },
    ///              "to": {
    ///                "description": "Target EVM address (0x…). Only on eth_sendTransaction.",
    ///                "type": "string"
    ///              },
    ///              "value": {
    ///                "description": "Wei amount as a string. Only on eth_sendTransaction.",
    ///                "type": "string"
    ///              }
    ///            }
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "description": "Always `eip1193_request` today.",
    ///      "type": "string"
    ///    },
    ///    "waitForReceipt": {
    ///      "description": "True iff the host should wait for the tx receipt before continuing to the next approval.",
    ///      "type": "boolean"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct BuildDepositResponseApprovalsItem {
        ///True iff this approval IS the deposit transaction (the final one in the array). Other entries are wallet chain switch + ERC-20 approvals.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub deposit: ::std::option::Option<bool>,
        pub request: BuildDepositResponseApprovalsItemRequest,
        ///Always `eip1193_request` today.
        #[serde(rename = "type")]
        pub type_: ::std::string::String,
        ///True iff the host should wait for the tx receipt before continuing to the next approval.
        #[serde(
            rename = "waitForReceipt",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub wait_for_receipt: ::std::option::Option<bool>,
    }
    ///`BuildDepositResponseApprovalsItemRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "method",
    ///    "params"
    ///  ],
    ///  "properties": {
    ///    "method": {
    ///      "description": "EIP-1193 method. Common values:\n  - `wallet_switchEthereumChain` (params: [{chainId}])\n  - `eth_sendTransaction`        (params: [{from,to,data,value?,chainId?}])\n",
    ///      "type": "string"
    ///    },
    ///    "params": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "properties": {
    ///          "chainId": {
    ///            "description": "Hex chain id (e.g. \"0x1\" for Ethereum). Present on wallet_switchEthereumChain AND eth_sendTransaction.",
    ///            "type": "string"
    ///          },
    ///          "data": {
    ///            "description": "0x-prefixed calldata. Only on eth_sendTransaction.",
    ///            "type": "string"
    ///          },
    ///          "from": {
    ///            "description": "Sender EVM address (0x…). Only on eth_sendTransaction.",
    ///            "type": "string"
    ///          },
    ///          "gas": {
    ///            "description": "Optional gas limit hint as a string.",
    ///            "type": "string"
    ///          },
    ///          "to": {
    ///            "description": "Target EVM address (0x…). Only on eth_sendTransaction.",
    ///            "type": "string"
    ///          },
    ///          "value": {
    ///            "description": "Wei amount as a string. Only on eth_sendTransaction.",
    ///            "type": "string"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct BuildDepositResponseApprovalsItemRequest {
        /**EIP-1193 method. Common values:
          - `wallet_switchEthereumChain` (params: [{chainId}])
          - `eth_sendTransaction`        (params: [{from,to,data,value?,chainId?}])
        */
        pub method: ::std::string::String,
        pub params: ::std::vec::Vec<BuildDepositResponseApprovalsItemRequestParamsItem>,
    }
    ///`BuildDepositResponseApprovalsItemRequestParamsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "chainId": {
    ///      "description": "Hex chain id (e.g. \"0x1\" for Ethereum). Present on wallet_switchEthereumChain AND eth_sendTransaction.",
    ///      "type": "string"
    ///    },
    ///    "data": {
    ///      "description": "0x-prefixed calldata. Only on eth_sendTransaction.",
    ///      "type": "string"
    ///    },
    ///    "from": {
    ///      "description": "Sender EVM address (0x…). Only on eth_sendTransaction.",
    ///      "type": "string"
    ///    },
    ///    "gas": {
    ///      "description": "Optional gas limit hint as a string.",
    ///      "type": "string"
    ///    },
    ///    "to": {
    ///      "description": "Target EVM address (0x…). Only on eth_sendTransaction.",
    ///      "type": "string"
    ///    },
    ///    "value": {
    ///      "description": "Wei amount as a string. Only on eth_sendTransaction.",
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct BuildDepositResponseApprovalsItemRequestParamsItem {
        ///Hex chain id (e.g. "0x1" for Ethereum). Present on wallet_switchEthereumChain AND eth_sendTransaction.
        #[serde(
            rename = "chainId",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub chain_id: ::std::option::Option<::std::string::String>,
        ///0x-prefixed calldata. Only on eth_sendTransaction.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub data: ::std::option::Option<::std::string::String>,
        ///Sender EVM address (0x…). Only on eth_sendTransaction.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub from: ::std::option::Option<::std::string::String>,
        ///Optional gas limit hint as a string.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub gas: ::std::option::Option<::std::string::String>,
        ///Target EVM address (0x…). Only on eth_sendTransaction.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub to: ::std::option::Option<::std::string::String>,
        ///Wei amount as a string. Only on eth_sendTransaction.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub value: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for BuildDepositResponseApprovalsItemRequestParamsItem {
        fn default() -> Self {
            Self {
                chain_id: Default::default(),
                data: Default::default(),
                from: Default::default(),
                gas: Default::default(),
                to: Default::default(),
                value: Default::default(),
            }
        }
    }
    /**Viem-style chain config. Keys vary across chains; only `id` and `name`
    are reliably present.
    */
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Viem-style chain config. Keys vary across chains; only `id` and `name`\nare reliably present.\n",
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "nativeCurrency": {
    ///      "type": "object",
    ///      "properties": {
    ///        "decimals": {
    ///          "type": "integer"
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        },
    ///        "symbol": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Chain {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "nativeCurrency",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub native_currency: ::std::option::Option<ChainNativeCurrency>,
    }
    impl ::std::default::Default for Chain {
        fn default() -> Self {
            Self {
                id: Default::default(),
                name: Default::default(),
                native_currency: Default::default(),
            }
        }
    }
    ///`ChainNativeCurrency`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "decimals": {
    ///      "type": "integer"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "symbol": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ChainNativeCurrency {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub decimals: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub symbol: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ChainNativeCurrency {
        fn default() -> Self {
            Self {
                decimals: Default::default(),
                name: Default::default(),
                symbol: Default::default(),
            }
        }
    }
    /**One of four request shapes; server dispatches by which fields are present.
    See `info.description` and `apps/khalani/src/types.rs` for the canonical
    / fromAddress / userAddress / legacy variants.
    */
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "One of four request shapes; server dispatches by which fields are present.\nSee `info.description` and `apps/khalani/src/types.rs` for the canonical\n/ fromAddress / userAddress / legacy variants.\n",
    ///  "type": "object",
    ///  "properties": {
    ///    "allowanceTarget": {
    ///      "type": "string"
    ///    },
    ///    "depositMethod": {
    ///      "type": "string"
    ///    },
    ///    "from": {
    ///      "type": "string"
    ///    },
    ///    "fromAddress": {
    ///      "type": "string"
    ///    },
    ///    "quoteId": {
    ///      "type": "string"
    ///    },
    ///    "routeId": {
    ///      "type": "string"
    ///    },
    ///    "slippageInBps": {
    ///      "type": "integer",
    ///      "maximum": 10000.0
    ///    },
    ///    "user": {
    ///      "type": "string"
    ///    },
    ///    "userAddress": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct DepositBuildRequest {
        #[serde(
            rename = "allowanceTarget",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub allowance_target: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "depositMethod",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub deposit_method: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub from: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "fromAddress",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub from_address: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "quoteId",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub quote_id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "routeId",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub route_id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "slippageInBps",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub slippage_in_bps: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub user: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "userAddress",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub user_address: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for DepositBuildRequest {
        fn default() -> Self {
            Self {
                allowance_target: Default::default(),
                deposit_method: Default::default(),
                from: Default::default(),
                from_address: Default::default(),
                quote_id: Default::default(),
                route_id: Default::default(),
                slippage_in_bps: Default::default(),
                user: Default::default(),
                user_address: Default::default(),
            }
        }
    }
    /**One of three request shapes (signed-EIP-712 / signed-transaction / legacy);
    server dispatches by which fields are present.
    */
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "One of three request shapes (signed-EIP-712 / signed-transaction / legacy);\nserver dispatches by which fields are present.\n",
    ///  "type": "object",
    ///  "properties": {
    ///    "quoteId": {
    ///      "type": "string"
    ///    },
    ///    "routeId": {
    ///      "type": "string"
    ///    },
    ///    "signature": {
    ///      "description": "Hex EIP-712 signature.",
    ///      "type": "string"
    ///    },
    ///    "submittedData": {
    ///      "type": "object",
    ///      "additionalProperties": true
    ///    },
    ///    "transactionHash": {
    ///      "type": "string"
    ///    },
    ///    "txHash": {
    ///      "type": "string"
    ///    }
    ///  },
    ///  "additionalProperties": true
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct DepositSubmitRequest {
        #[serde(
            rename = "quoteId",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub quote_id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "routeId",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub route_id: ::std::option::Option<::std::string::String>,
        ///Hex EIP-712 signature.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub signature: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "submittedData",
            default,
            skip_serializing_if = "::serde_json::Map::is_empty"
        )]
        pub submitted_data: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(
            rename = "transactionHash",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub transaction_hash: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "txHash",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub tx_hash: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for DepositSubmitRequest {
        fn default() -> Self {
            Self {
                quote_id: Default::default(),
                route_id: Default::default(),
                signature: Default::default(),
                submitted_data: Default::default(),
                transaction_hash: Default::default(),
                tx_hash: Default::default(),
            }
        }
    }
    ///`GetOrdersByAddressResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "data"
    ///  ],
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "additionalProperties": true
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetOrdersByAddressResponse {
        pub data: ::std::vec::Vec<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    }
    ///`GetQuoteResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "quoteId",
    ///    "routes"
    ///  ],
    ///  "properties": {
    ///    "quoteId": {
    ///      "type": "string"
    ///    },
    ///    "routes": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "required": [
    ///          "depositMethods",
    ///          "exactOutMethod",
    ///          "icon",
    ///          "quote",
    ///          "routeId",
    ///          "type"
    ///        ],
    ///        "properties": {
    ///          "depositMethods": {
    ///            "type": "array",
    ///            "items": {
    ///              "type": "string"
    ///            }
    ///          },
    ///          "exactOutMethod": {
    ///            "type": "string"
    ///          },
    ///          "icon": {
    ///            "type": "string"
    ///          },
    ///          "quote": {
    ///            "type": "object",
    ///            "required": [
    ///              "amountIn",
    ///              "amountOut",
    ///              "expectedDurationSeconds",
    ///              "quoteExpiresAt",
    ///              "tags",
    ///              "validBefore"
    ///            ],
    ///            "properties": {
    ///              "amountIn": {
    ///                "type": "string"
    ///              },
    ///              "amountOut": {
    ///                "type": "string"
    ///              },
    ///              "expectedDurationSeconds": {
    ///                "type": "integer"
    ///              },
    ///              "quoteExpiresAt": {
    ///                "type": "integer"
    ///              },
    ///              "supportedDepositMethods": {
    ///                "type": "array",
    ///                "items": {
    ///                  "type": "string"
    ///                }
    ///              },
    ///              "tags": {
    ///                "type": "array",
    ///                "items": {
    ///                  "type": "string"
    ///                }
    ///              },
    ///              "validBefore": {
    ///                "type": "integer"
    ///              }
    ///            }
    ///          },
    ///          "routeId": {
    ///            "type": "string"
    ///          },
    ///          "type": {
    ///            "type": "string"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetQuoteResponse {
        #[serde(rename = "quoteId")]
        pub quote_id: ::std::string::String,
        pub routes: ::std::vec::Vec<GetQuoteResponseRoutesItem>,
    }
    ///`GetQuoteResponseRoutesItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "depositMethods",
    ///    "exactOutMethod",
    ///    "icon",
    ///    "quote",
    ///    "routeId",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "depositMethods": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "exactOutMethod": {
    ///      "type": "string"
    ///    },
    ///    "icon": {
    ///      "type": "string"
    ///    },
    ///    "quote": {
    ///      "type": "object",
    ///      "required": [
    ///        "amountIn",
    ///        "amountOut",
    ///        "expectedDurationSeconds",
    ///        "quoteExpiresAt",
    ///        "tags",
    ///        "validBefore"
    ///      ],
    ///      "properties": {
    ///        "amountIn": {
    ///          "type": "string"
    ///        },
    ///        "amountOut": {
    ///          "type": "string"
    ///        },
    ///        "expectedDurationSeconds": {
    ///          "type": "integer"
    ///        },
    ///        "quoteExpiresAt": {
    ///          "type": "integer"
    ///        },
    ///        "supportedDepositMethods": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "tags": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "validBefore": {
    ///          "type": "integer"
    ///        }
    ///      }
    ///    },
    ///    "routeId": {
    ///      "type": "string"
    ///    },
    ///    "type": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetQuoteResponseRoutesItem {
        #[serde(rename = "depositMethods")]
        pub deposit_methods: ::std::vec::Vec<::std::string::String>,
        #[serde(rename = "exactOutMethod")]
        pub exact_out_method: ::std::string::String,
        pub icon: ::std::string::String,
        pub quote: GetQuoteResponseRoutesItemQuote,
        #[serde(rename = "routeId")]
        pub route_id: ::std::string::String,
        #[serde(rename = "type")]
        pub type_: ::std::string::String,
    }
    ///`GetQuoteResponseRoutesItemQuote`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "amountIn",
    ///    "amountOut",
    ///    "expectedDurationSeconds",
    ///    "quoteExpiresAt",
    ///    "tags",
    ///    "validBefore"
    ///  ],
    ///  "properties": {
    ///    "amountIn": {
    ///      "type": "string"
    ///    },
    ///    "amountOut": {
    ///      "type": "string"
    ///    },
    ///    "expectedDurationSeconds": {
    ///      "type": "integer"
    ///    },
    ///    "quoteExpiresAt": {
    ///      "type": "integer"
    ///    },
    ///    "supportedDepositMethods": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "validBefore": {
    ///      "type": "integer"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetQuoteResponseRoutesItemQuote {
        #[serde(rename = "amountIn")]
        pub amount_in: ::std::string::String,
        #[serde(rename = "amountOut")]
        pub amount_out: ::std::string::String,
        #[serde(rename = "expectedDurationSeconds")]
        pub expected_duration_seconds: i64,
        #[serde(rename = "quoteExpiresAt")]
        pub quote_expires_at: i64,
        #[serde(
            rename = "supportedDepositMethods",
            default,
            skip_serializing_if = "::std::vec::Vec::is_empty"
        )]
        pub supported_deposit_methods: ::std::vec::Vec<::std::string::String>,
        pub tags: ::std::vec::Vec<::std::string::String>,
        #[serde(rename = "validBefore")]
        pub valid_before: i64,
    }
    ///`ListTokensResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Token"
    ///      }
    ///    },
    ///    {
    ///      "type": "object",
    ///      "properties": {
    ///        "data": {
    ///          "type": "array",
    ///          "items": {
    ///            "$ref": "#/components/schemas/Token"
    ///          }
    ///        }
    ///      }
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum ListTokensResponse {
        Array(::std::vec::Vec<Token>),
        Object {
            #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
            data: ::std::vec::Vec<Token>,
        },
    }
    impl ::std::convert::From<::std::vec::Vec<Token>> for ListTokensResponse {
        fn from(value: ::std::vec::Vec<Token>) -> Self {
            Self::Array(value)
        }
    }
    ///`QuoteRequest`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "amount",
    ///    "fromAddress",
    ///    "fromChainId",
    ///    "fromToken",
    ///    "toChainId",
    ///    "toToken",
    ///    "tradeType"
    ///  ],
    ///  "properties": {
    ///    "amount": {
    ///      "description": "Base-units string (no decimal point), e.g. \"1000000\".",
    ///      "type": "string"
    ///    },
    ///    "fromAddress": {
    ///      "type": "string"
    ///    },
    ///    "fromChainId": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "fromToken": {
    ///      "description": "0x-address (or \"native\" sentinel).",
    ///      "type": "string"
    ///    },
    ///    "recipient": {
    ///      "description": "Defaults to fromAddress when omitted.",
    ///      "type": "string"
    ///    },
    ///    "slippageInBps": {
    ///      "description": "Slippage tolerance in basis points (50 = 0.5%).",
    ///      "type": "integer",
    ///      "maximum": 10000.0,
    ///      "minimum": 0.0
    ///    },
    ///    "toChainId": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "toToken": {
    ///      "type": "string"
    ///    },
    ///    "tradeType": {
    ///      "description": "Almost always EXACT_INPUT.",
    ///      "type": "string",
    ///      "enum": [
    ///        "EXACT_INPUT",
    ///        "EXACT_OUTPUT"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct QuoteRequest {
        ///Base-units string (no decimal point), e.g. "1000000".
        pub amount: ::std::string::String,
        #[serde(rename = "fromAddress")]
        pub from_address: ::std::string::String,
        #[serde(rename = "fromChainId")]
        pub from_chain_id: i64,
        ///0x-address (or "native" sentinel).
        #[serde(rename = "fromToken")]
        pub from_token: ::std::string::String,
        ///Defaults to fromAddress when omitted.
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub recipient: ::std::option::Option<::std::string::String>,
        ///Slippage tolerance in basis points (50 = 0.5%).
        #[serde(
            rename = "slippageInBps",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub slippage_in_bps: ::std::option::Option<i64>,
        #[serde(rename = "toChainId")]
        pub to_chain_id: i64,
        #[serde(rename = "toToken")]
        pub to_token: ::std::string::String,
        ///Almost always EXACT_INPUT.
        #[serde(rename = "tradeType")]
        pub trade_type: QuoteRequestTradeType,
    }
    ///Almost always EXACT_INPUT.
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Almost always EXACT_INPUT.",
    ///  "type": "string",
    ///  "enum": [
    ///    "EXACT_INPUT",
    ///    "EXACT_OUTPUT"
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(
        ::serde::Deserialize,
        ::serde::Serialize,
        Clone,
        Copy,
        Debug,
        Eq,
        Hash,
        Ord,
        PartialEq,
        PartialOrd,
    )]
    pub enum QuoteRequestTradeType {
        #[serde(rename = "EXACT_INPUT")]
        ExactInput,
        #[serde(rename = "EXACT_OUTPUT")]
        ExactOutput,
    }
    impl ::std::fmt::Display for QuoteRequestTradeType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::ExactInput => f.write_str("EXACT_INPUT"),
                Self::ExactOutput => f.write_str("EXACT_OUTPUT"),
            }
        }
    }
    impl ::std::str::FromStr for QuoteRequestTradeType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "EXACT_INPUT" => Ok(Self::ExactInput),
                "EXACT_OUTPUT" => Ok(Self::ExactOutput),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for QuoteRequestTradeType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for QuoteRequestTradeType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for QuoteRequestTradeType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`SearchTokensResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Token"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct SearchTokensResponse {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<Token>,
    }
    impl ::std::default::Default for SearchTokensResponse {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`Token`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "address",
    ///    "chainId",
    ///    "decimals",
    ///    "symbol"
    ///  ],
    ///  "properties": {
    ///    "address": {
    ///      "description": "Lowercase hex address.",
    ///      "type": "string"
    ///    },
    ///    "chainId": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "decimals": {
    ///      "type": "integer",
    ///      "maximum": 36.0
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "symbol": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Token {
        ///Lowercase hex address.
        pub address: ::std::string::String,
        #[serde(rename = "chainId")]
        pub chain_id: i64,
        pub decimals: i64,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        pub symbol: ::std::string::String,
    }
}
#[derive(Clone, Debug)]
/**Client for Khalani Hyperstream API

Khalani's cross-chain intent execution API at api.hyperstream.dev.

## Auth
No auth headers are required for the endpoints in this spec. Settlement
requires a user signature (EIP-712) but the signing is done client-side and
submitted via `/v1/deposit/submit` — there is no API key.

## Workflow
1. Call `/v1/quotes` with from/to chain+token and amount → get quote+route.
2. Call `/v1/deposit/build` with the chosen quote → get an EIP-712 typed-
   data payload OR raw transaction params for the user to sign.
3. Sign client-side, then PUT `/v1/deposit/submit` with the signature or
   transaction hash.
4. Poll `/v1/orders/{address}` for status.

Helper endpoints: `/v1/chains` (chain metadata), `/v1/tokens` (token list),
`/v1/tokens/search` (search by symbol/name/address).


Version: 1.0*/
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
        "1.0"
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
    /**Quote a cross-chain swap or transfer for a given pair and amount

    Sends a `POST` request to `/v1/quotes`

    */
    pub async fn get_quote<'a>(
        &'a self,
        body: &'a types::QuoteRequest,
    ) -> Result<ResponseValue<types::GetQuoteResponse>, Error<()>> {
        let url = format!("{}/v1/quotes", self.baseurl,);
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
            operation_id: "get_quote",
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
    /**Build the EIP-712 typed data or raw tx for a quoted route

    Accepts one of four request shapes (canonical / fromAddress / userAddress / legacy)
    depending on the deposit method. Modeled here as a permissive object — the actual
    shape is dispatched by the server based on which fields are present.


    Sends a `POST` request to `/v1/deposit/build`

    */
    pub async fn build_deposit<'a>(
        &'a self,
        body: &'a types::DepositBuildRequest,
    ) -> Result<ResponseValue<types::BuildDepositResponse>, Error<()>> {
        let url = format!("{}/v1/deposit/build", self.baseurl,);
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
            operation_id: "build_deposit",
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
    /**Submit a signed EIP-712 payload or a settled transaction hash

    Accepts one of three request shapes (signed-EIP-712 / signed-transaction / legacy).
    Modeled here as a permissive object dispatched by the server.


    Sends a `PUT` request to `/v1/deposit/submit`

    */
    pub async fn submit_deposit<'a>(
        &'a self,
        body: &'a types::DepositSubmitRequest,
    ) -> Result<
        ResponseValue<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        Error<()>,
    > {
        let url = format!("{}/v1/deposit/submit", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .put(url)
            .header(
                ::reqwest::header::ACCEPT,
                ::reqwest::header::HeaderValue::from_static("application/json"),
            )
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "submit_deposit",
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
    /**List recent orders for a user address

    Sends a `GET` request to `/v1/orders/{address}`

    Arguments:
    - `address`: EVM address (0x-prefixed, 20 bytes).
    - `limit`
    - `offset`
    - `order_ids`: Comma-separated order IDs to filter to.
    - `status`
    */
    pub async fn get_orders_by_address<'a>(
        &'a self,
        address: &'a str,
        limit: Option<i64>,
        offset: Option<i64>,
        order_ids: Option<&'a str>,
        status: Option<&'a str>,
    ) -> Result<ResponseValue<types::GetOrdersByAddressResponse>, Error<()>> {
        let url = format!(
            "{}/v1/orders/{}",
            self.baseurl,
            encode_path(&address.to_string()),
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
            .query(&progenitor_client::QueryParam::new("limit", &limit))
            .query(&progenitor_client::QueryParam::new("offset", &offset))
            .query(&progenitor_client::QueryParam::new("orderIds", &order_ids))
            .query(&progenitor_client::QueryParam::new("status", &status))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "get_orders_by_address",
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
    /**List supported tokens, optionally filtered by chain or substring

    Sends a `GET` request to `/v1/tokens`

    Arguments:
    - `chain_ids`: Single chain id (despite the plural name).
    - `limit`
    - `offset`
    - `q`: Substring match on symbol/name/address.
    */
    pub async fn list_tokens<'a>(
        &'a self,
        chain_ids: Option<i64>,
        limit: Option<i64>,
        offset: Option<i64>,
        q: Option<&'a str>,
    ) -> Result<ResponseValue<types::ListTokensResponse>, Error<()>> {
        let url = format!("{}/v1/tokens", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("chainIds", &chain_ids))
            .query(&progenitor_client::QueryParam::new("limit", &limit))
            .query(&progenitor_client::QueryParam::new("offset", &offset))
            .query(&progenitor_client::QueryParam::new("q", &q))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "list_tokens",
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
    /**Search tokens by symbol/name/address — required `q` query

    Sends a `GET` request to `/v1/tokens/search`

    */
    pub async fn search_tokens<'a>(
        &'a self,
        chain_ids: Option<i64>,
        limit: Option<i64>,
        offset: Option<i64>,
        q: &'a str,
    ) -> Result<ResponseValue<types::SearchTokensResponse>, Error<()>> {
        let url = format!("{}/v1/tokens/search", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("chainIds", &chain_ids))
            .query(&progenitor_client::QueryParam::new("limit", &limit))
            .query(&progenitor_client::QueryParam::new("offset", &offset))
            .query(&progenitor_client::QueryParam::new("q", &q))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "search_tokens",
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
    /**List supported chains with viem-style metadata

    Sends a `GET` request to `/v1/chains`

    */
    pub async fn list_chains<'a>(
        &'a self,
    ) -> Result<ResponseValue<::std::vec::Vec<types::Chain>>, Error<()>> {
        let url = format!("{}/v1/chains", self.baseurl,);
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
            operation_id: "list_chains",
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
