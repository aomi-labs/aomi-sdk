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
    ///`DexesList`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": [
    ///        {
    ///          "attributes": {
    ///            "name": "Uniswap V2"
    ///          },
    ///          "id": "uniswap_v2",
    ///          "type": "dex"
    ///        },
    ///        {
    ///          "attributes": {
    ///            "name": "SushiSwap"
    ///          },
    ///          "id": "sushiswap",
    ///          "type": "dex"
    ///        },
    ///        {
    ///          "attributes": {
    ///            "name": "Uniswap V3"
    ///          },
    ///          "id": "uniswap_v3",
    ///          "type": "dex"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "properties": {
    ///          "attributes": {
    ///            "type": "object",
    ///            "properties": {
    ///              "name": {
    ///                "type": "string"
    ///              }
    ///            }
    ///          },
    ///          "id": {
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
    pub struct DexesList {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<DexesListDataItem>,
    }
    impl ::std::default::Default for DexesList {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`DexesListDataItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "attributes": {
    ///      "type": "object",
    ///      "properties": {
    ///        "name": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "id": {
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
    pub struct DexesListDataItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub attributes: ::std::option::Option<DexesListDataItemAttributes>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for DexesListDataItem {
        fn default() -> Self {
            Self {
                attributes: Default::default(),
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`DexesListDataItemAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "name": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct DexesListDataItemAttributes {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for DexesListDataItemAttributes {
        fn default() -> Self {
            Self {
                name: Default::default(),
            }
        }
    }
    ///`ErrorsObject`
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
    ///        "type": "object",
    ///        "properties": {
    ///          "status": {
    ///            "type": "string"
    ///          },
    ///          "title": {
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
    pub struct ErrorsObject {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub errors: ::std::vec::Vec<ErrorsObjectErrorsItem>,
    }
    impl ::std::default::Default for ErrorsObject {
        fn default() -> Self {
            Self {
                errors: Default::default(),
            }
        }
    }
    ///`ErrorsObjectErrorsItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "status": {
    ///      "type": "string"
    ///    },
    ///    "title": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct ErrorsObjectErrorsItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub title: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ErrorsObjectErrorsItem {
        fn default() -> Self {
            Self {
                status: Default::default(),
                title: Default::default(),
            }
        }
    }
    ///`GetnetworksNetworkDexesDexPoolsSort`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "h24_volume_usd_desc",
    ///    "h24_tx_count_desc"
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
    pub enum GetnetworksNetworkDexesDexPoolsSort {
        #[serde(rename = "h24_volume_usd_desc")]
        H24VolumeUsdDesc,
        #[serde(rename = "h24_tx_count_desc")]
        H24TxCountDesc,
    }
    impl ::std::fmt::Display for GetnetworksNetworkDexesDexPoolsSort {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::H24VolumeUsdDesc => f.write_str("h24_volume_usd_desc"),
                Self::H24TxCountDesc => f.write_str("h24_tx_count_desc"),
            }
        }
    }
    impl ::std::str::FromStr for GetnetworksNetworkDexesDexPoolsSort {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "h24_volume_usd_desc" => Ok(Self::H24VolumeUsdDesc),
                "h24_tx_count_desc" => Ok(Self::H24TxCountDesc),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for GetnetworksNetworkDexesDexPoolsSort {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for GetnetworksNetworkDexesDexPoolsSort {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for GetnetworksNetworkDexesDexPoolsSort {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "usd",
    ///  "type": "string",
    ///  "enum": [
    ///    "usd",
    ///    "token"
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
    pub enum GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency {
        #[serde(rename = "usd")]
        Usd,
        #[serde(rename = "token")]
        Token,
    }
    impl ::std::fmt::Display for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Usd => f.write_str("usd"),
                Self::Token => f.write_str("token"),
            }
        }
    }
    impl ::std::str::FromStr for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "usd" => Ok(Self::Usd),
                "token" => Ok(Self::Token),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
        for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
        for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::default::Default for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency {
        fn default() -> Self {
            GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency::Usd
        }
    }
    ///`GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "day",
    ///    "hour",
    ///    "minute"
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
    pub enum GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe {
        #[serde(rename = "day")]
        Day,
        #[serde(rename = "hour")]
        Hour,
        #[serde(rename = "minute")]
        Minute,
    }
    impl ::std::fmt::Display for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Day => f.write_str("day"),
                Self::Hour => f.write_str("hour"),
                Self::Minute => f.write_str("minute"),
            }
        }
    }
    impl ::std::str::FromStr for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "day" => Ok(Self::Day),
                "hour" => Ok(Self::Hour),
                "minute" => Ok(Self::Minute),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
        for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
        for GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`GetnetworksNetworkPoolsSort`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "h24_volume_usd_desc",
    ///    "h24_tx_count_desc"
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
    pub enum GetnetworksNetworkPoolsSort {
        #[serde(rename = "h24_volume_usd_desc")]
        H24VolumeUsdDesc,
        #[serde(rename = "h24_tx_count_desc")]
        H24TxCountDesc,
    }
    impl ::std::fmt::Display for GetnetworksNetworkPoolsSort {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::H24VolumeUsdDesc => f.write_str("h24_volume_usd_desc"),
                Self::H24TxCountDesc => f.write_str("h24_tx_count_desc"),
            }
        }
    }
    impl ::std::str::FromStr for GetnetworksNetworkPoolsSort {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "h24_volume_usd_desc" => Ok(Self::H24VolumeUsdDesc),
                "h24_tx_count_desc" => Ok(Self::H24TxCountDesc),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for GetnetworksNetworkPoolsSort {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for GetnetworksNetworkPoolsSort {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for GetnetworksNetworkPoolsSort {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`GetnetworksNetworkTokensTokenaddressPoolsSort`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "h24_volume_usd_desc",
    ///    "h24_tx_count_desc",
    ///    "h24_volume_usd_liquidity_desc"
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
    pub enum GetnetworksNetworkTokensTokenaddressPoolsSort {
        #[serde(rename = "h24_volume_usd_desc")]
        H24VolumeUsdDesc,
        #[serde(rename = "h24_tx_count_desc")]
        H24TxCountDesc,
        #[serde(rename = "h24_volume_usd_liquidity_desc")]
        H24VolumeUsdLiquidityDesc,
    }
    impl ::std::fmt::Display for GetnetworksNetworkTokensTokenaddressPoolsSort {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::H24VolumeUsdDesc => f.write_str("h24_volume_usd_desc"),
                Self::H24TxCountDesc => f.write_str("h24_tx_count_desc"),
                Self::H24VolumeUsdLiquidityDesc => f.write_str("h24_volume_usd_liquidity_desc"),
            }
        }
    }
    impl ::std::str::FromStr for GetnetworksNetworkTokensTokenaddressPoolsSort {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "h24_volume_usd_desc" => Ok(Self::H24VolumeUsdDesc),
                "h24_tx_count_desc" => Ok(Self::H24TxCountDesc),
                "h24_volume_usd_liquidity_desc" => Ok(Self::H24VolumeUsdLiquidityDesc),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for GetnetworksNetworkTokensTokenaddressPoolsSort {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String>
        for GetnetworksNetworkTokensTokenaddressPoolsSort
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String>
        for GetnetworksNetworkTokensTokenaddressPoolsSort
    {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`GetnetworksNetworkTrendingpoolsDuration`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "5m",
    ///    "1h",
    ///    "6h",
    ///    "24h"
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
    pub enum GetnetworksNetworkTrendingpoolsDuration {
        #[serde(rename = "5m")]
        X5m,
        #[serde(rename = "1h")]
        X1h,
        #[serde(rename = "6h")]
        X6h,
        #[serde(rename = "24h")]
        X24h,
    }
    impl ::std::fmt::Display for GetnetworksNetworkTrendingpoolsDuration {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X5m => f.write_str("5m"),
                Self::X1h => f.write_str("1h"),
                Self::X6h => f.write_str("6h"),
                Self::X24h => f.write_str("24h"),
            }
        }
    }
    impl ::std::str::FromStr for GetnetworksNetworkTrendingpoolsDuration {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "5m" => Ok(Self::X5m),
                "1h" => Ok(Self::X1h),
                "6h" => Ok(Self::X6h),
                "24h" => Ok(Self::X24h),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for GetnetworksNetworkTrendingpoolsDuration {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for GetnetworksNetworkTrendingpoolsDuration {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for GetnetworksNetworkTrendingpoolsDuration {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`GetnetworksTrendingpoolsDuration`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "5m",
    ///    "1h",
    ///    "6h",
    ///    "24h"
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
    pub enum GetnetworksTrendingpoolsDuration {
        #[serde(rename = "5m")]
        X5m,
        #[serde(rename = "1h")]
        X1h,
        #[serde(rename = "6h")]
        X6h,
        #[serde(rename = "24h")]
        X24h,
    }
    impl ::std::fmt::Display for GetnetworksTrendingpoolsDuration {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::X5m => f.write_str("5m"),
                Self::X1h => f.write_str("1h"),
                Self::X6h => f.write_str("6h"),
                Self::X24h => f.write_str("24h"),
            }
        }
    }
    impl ::std::str::FromStr for GetnetworksTrendingpoolsDuration {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "5m" => Ok(Self::X5m),
                "1h" => Ok(Self::X1h),
                "6h" => Ok(Self::X6h),
                "24h" => Ok(Self::X24h),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for GetnetworksTrendingpoolsDuration {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for GetnetworksTrendingpoolsDuration {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for GetnetworksTrendingpoolsDuration {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`GetsimpleNetworksNetworkTokenpriceAddressesResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/simple_token_price"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct GetsimpleNetworksNetworkTokenpriceAddressesResponse {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<SimpleTokenPrice>,
    }
    impl ::std::default::Default for GetsimpleNetworksNetworkTokenpriceAddressesResponse {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`MultiPoolDetails`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": [
    ///        {
    ///          "attributes": {
    ///            "address": "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
    ///            "base_token_balance": "5783.57638014946",
    ///            "base_token_liquidity_usd": "25794285.689214855",
    ///            "base_token_price_native_currency": "1.0",
    ///            "base_token_price_quote_token": "4467.038132299",
    ///            "base_token_price_usd": "4459.91960575582",
    ///            "buy_volume_usd": {
    ///              "h1": "1433790.04000455",
    ///              "h24": "8012516.79734659",
    ///              "h6": "2569275.25209895",
    ///              "m15": "90542.361944655",
    ///              "m30": "166769.501005617",
    ///              "m5": "0.0"
    ///            },
    ///            "fdv_usd": "11160006028.1758",
    ///            "locked_liquidity_percentage": "0.0",
    ///            "market_cap_usd": "11166433701.589",
    ///            "name": "WETH / USDC 0.05%",
    ///            "net_buy_volume_usd": {
    ///              "h1": "66459.25132413",
    ///              "h24": "-165843.81722825",
    ///              "h6": "-336894.73878909",
    ///              "m15": "-106958.58061148",
    ///              "m30": "-32014.899699526",
    ///              "m5": "-37399.6069733459"
    ///            },
    ///            "pool_created_at": "2021-12-29T12:35:14Z",
    ///            "pool_fee_percentage": "0.05",
    ///            "pool_name": "WETH / USDC",
    ///            "price_change_percentage": {
    ///              "h1": "0.039",
    ///              "h24": "-0.221",
    ///              "h6": "-0.435",
    ///              "m15": "0.027",
    ///              "m30": "0.028",
    ///              "m5": "0"
    ///            },
    ///            "quote_token_balance": "64655288.355199",
    ///            "quote_token_liquidity_usd": "64552255.7925259",
    ///            "quote_token_price_base_token": "0.0002238619798",
    ///            "quote_token_price_native_currency": "0.000223861979768972",
    ///            "quote_token_price_usd": "0.998614899014123",
    ///            "reserve_in_usd": "90346541.3306",
    ///            "sell_volume_usd": {
    ///              "h1": "1367330.78868042",
    ///              "h24": "8178360.61457484",
    ///              "h6": "2906169.99088804",
    ///              "m15": "197500.942556135",
    ///              "m30": "198784.400705143",
    ///              "m5": "37399.6069733459"
    ///            },
    ///            "transactions": {
    ///              "h1": {
    ///                "buyers": 56,
    ///                "buys": 86,
    ///                "sellers": 79,
    ///                "sells": 104
    ///              },
    ///              "h24": {
    ///                "buyers": 740,
    ///                "buys": 1517,
    ///                "sellers": 1302,
    ///                "sells": 1828
    ///              },
    ///              "h6": {
    ///                "buyers": 212,
    ///                "buys": 383,
    ///                "sellers": 388,
    ///                "sells": 524
    ///              },
    ///              "m15": {
    ///                "buyers": 9,
    ///                "buys": 9,
    ///                "sellers": 23,
    ///                "sells": 27
    ///              },
    ///              "m30": {
    ///                "buyers": 18,
    ///                "buys": 23,
    ///                "sellers": 37,
    ///                "sells": 43
    ///              },
    ///              "m5": {
    ///                "buyers": 0,
    ///                "buys": 0,
    ///                "sellers": 4,
    ///                "sells": 4
    ///              }
    ///            },
    ///            "volume_usd": {
    ///              "h1": "2801120.82868497",
    ///              "h24": "16190877.4119214",
    ///              "h6": "5475445.24298699",
    ///              "m15": "288043.30450079",
    ///              "m30": "365553.90171076",
    ///              "m5": "37399.6069733459"
    ///            }
    ///          },
    ///          "id": "eth_0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
    ///          "relationships": {
    ///            "base_token": {
    ///              "data": {
    ///                "id": "eth_0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///                "type": "token"
    ///              }
    ///            },
    ///            "dex": {
    ///              "data": {
    ///                "id": "uniswap_v3",
    ///                "type": "dex"
    ///              }
    ///            },
    ///            "quote_token": {
    ///              "data": {
    ///                "id": "eth_0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    ///                "type": "token"
    ///              }
    ///            }
    ///          },
    ///          "type": "pool"
    ///        }
    ///      ],
    ///      "included": [
    ///        {
    ///          "attributes": {
    ///            "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///            "coingecko_coin_id": "weth",
    ///            "decimals": 18,
    ///            "image_url": "https://coin-images.coingecko.com/coins/images/2518/large/weth.png?1696503332",
    ///            "name": "Wrapped Ether",
    ///            "symbol": "WETH"
    ///          },
    ///          "id": "eth_0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///          "type": "token"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/pool_details"
    ///      }
    ///    },
    ///    "included": {
    ///      "$ref": "#/components/schemas/pools_included"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct MultiPoolDetails {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<PoolDetails>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub included: ::std::option::Option<PoolsIncluded>,
    }
    impl ::std::default::Default for MultiPoolDetails {
        fn default() -> Self {
            Self {
                data: Default::default(),
                included: Default::default(),
            }
        }
    }
    ///`MultiTokenDetail`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": [
    ///        {
    ///          "attributes": {
    ///            "address": "0x44ff8620b8ca30902395a7bd3f2407e1a091bf73",
    ///            "banner_image_url": null,
    ///            "coingecko_coin_id": "virtual-protocol",
    ///            "decimals": 18,
    ///            "fdv_usd": "1236049062.71205",
    ///            "image_url": "https://coin-images.coingecko.com/coins/images/34057/large/LOGOMARK.png?1708356054",
    ///            "market_cap_usd": "810357802.002085",
    ///            "name": "Virtual Protocol",
    ///            "normalized_total_supply": "1000000000.0",
    ///            "price_usd": "1.2360509143",
    ///            "symbol": "VIRTUAL",
    ///            "total_reserve_in_usd": "1057431.7464185216625800287534040469",
    ///            "total_supply": "1000000000000000000000000000.0",
    ///            "volume_usd": {
    ///              "h24": "38713.9260903249"
    ///            }
    ///          },
    ///          "id": "eth_0x44ff8620b8ca30902395a7bd3f2407e1a091bf73",
    ///          "relationships": {
    ///            "top_pools": {
    ///              "data": [
    ///                {
    ///                  "id": "eth_0x95a45a87dd4d3a1803039072f37e075f37b23d75",
    ///                  "type": "pool"
    ///                }
    ///              ]
    ///            }
    ///          },
    ///          "type": "token"
    ///        }
    ///      ],
    ///      "included": [
    ///        {
    ///          "attributes": {
    ///            "address": "0x95a45a87dd4d3a1803039072f37e075f37b23d75",
    ///            "base_token_balance": "632455.064963526",
    ///            "base_token_liquidity_usd": "781765.7043993239",
    ///            "base_token_price_native_currency": "0.000276634218716478",
    ///            "base_token_price_quote_token": "0.0002766342187",
    ///            "base_token_price_usd": "1.23605091431632",
    ///            "fdv_usd": "1236049062.69573",
    ///            "market_cap_usd": "810357802.002085",
    ///            "name": "VIRTUAL / WETH 1%",
    ///            "pool_created_at": "2023-12-24T15:16:59Z",
    ///            "price_change_percentage": {
    ///              "h1": "-0.033",
    ///              "h24": "-3.945",
    ///              "h6": "-1.177",
    ///              "m15": "0",
    ///              "m30": "-0.01",
    ///              "m5": "0"
    ///            },
    ///            "quote_token_balance": "47.9170335228147",
    ///            "quote_token_liquidity_usd": "214107.04772415984",
    ///            "quote_token_price_base_token": "3614.881790979",
    ///            "quote_token_price_native_currency": "1.0",
    ///            "quote_token_price_usd": "4468.28678620552",
    ///            "reserve_in_usd": "995880.9039",
    ///            "transactions": {
    ///              "h1": {
    ///                "buyers": 0,
    ///                "buys": 0,
    ///                "sellers": 8,
    ///                "sells": 12
    ///              },
    ///              "h24": {
    ///                "buyers": 13,
    ///                "buys": 17,
    ///                "sellers": 44,
    ///                "sells": 68
    ///              },
    ///              "h6": {
    ///                "buyers": 4,
    ///                "buys": 5,
    ///                "sellers": 26,
    ///                "sells": 39
    ///              },
    ///              "m15": {
    ///                "buyers": 0,
    ///                "buys": 0,
    ///                "sellers": 0,
    ///                "sells": 0
    ///              },
    ///              "m30": {
    ///                "buyers": 0,
    ///                "buys": 0,
    ///                "sellers": 4,
    ///                "sells": 4
    ///              },
    ///              "m5": {
    ///                "buyers": 0,
    ///                "buys": 0,
    ///                "sellers": 0,
    ///                "sells": 0
    ///              }
    ///            },
    ///            "volume_usd": {
    ///              "h1": "349.000957995",
    ///              "h24": "21703.1874601209",
    ///              "h6": "15478.0378042767",
    ///              "m15": "0.0",
    ///              "m30": "124.147451535",
    ///              "m5": "0.0"
    ///            }
    ///          },
    ///          "id": "eth_0x95a45a87dd4d3a1803039072f37e075f37b23d75",
    ///          "relationships": {
    ///            "base_token": {
    ///              "data": {
    ///                "id": "eth_0x44ff8620b8ca30902395a7bd3f2407e1a091bf73",
    ///                "type": "token"
    ///              }
    ///            },
    ///            "dex": {
    ///              "data": {
    ///                "id": "uniswap_v3",
    ///                "type": "dex"
    ///              }
    ///            },
    ///            "quote_token": {
    ///              "data": {
    ///                "id": "eth_0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///                "type": "token"
    ///              }
    ///            }
    ///          },
    ///          "type": "pool"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/token"
    ///      }
    ///    },
    ///    "included": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/pool"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct MultiTokenDetail {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<Token>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub included: ::std::vec::Vec<Pool>,
    }
    impl ::std::default::Default for MultiTokenDetail {
        fn default() -> Self {
            Self {
                data: Default::default(),
                included: Default::default(),
            }
        }
    }
    ///`NetworksList`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": [
    ///        {
    ///          "attributes": {
    ///            "coingecko_asset_platform_id": "ethereum",
    ///            "name": "Ethereum"
    ///          },
    ///          "id": "eth",
    ///          "type": "network"
    ///        },
    ///        {
    ///          "attributes": {
    ///            "coingecko_asset_platform_id": "binance-smart-chain",
    ///            "name": "BNB Chain"
    ///          },
    ///          "id": "bsc",
    ///          "type": "network"
    ///        },
    ///        {
    ///          "attributes": {
    ///            "coingecko_asset_platform_id": "polygon-pos",
    ///            "name": "Polygon POS"
    ///          },
    ///          "id": "polygon_pos",
    ///          "type": "network"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "object",
    ///        "properties": {
    ///          "attributes": {
    ///            "type": "object",
    ///            "properties": {
    ///              "coingecko_asset_platform_id": {
    ///                "type": "string"
    ///              },
    ///              "name": {
    ///                "type": "string"
    ///              }
    ///            }
    ///          },
    ///          "id": {
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
    pub struct NetworksList {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<NetworksListDataItem>,
    }
    impl ::std::default::Default for NetworksList {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`NetworksListDataItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "attributes": {
    ///      "type": "object",
    ///      "properties": {
    ///        "coingecko_asset_platform_id": {
    ///          "type": "string"
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "id": {
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
    pub struct NetworksListDataItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub attributes: ::std::option::Option<NetworksListDataItemAttributes>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for NetworksListDataItem {
        fn default() -> Self {
            Self {
                attributes: Default::default(),
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`NetworksListDataItemAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "coingecko_asset_platform_id": {
    ///      "type": "string"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct NetworksListDataItemAttributes {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub coingecko_asset_platform_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for NetworksListDataItemAttributes {
        fn default() -> Self {
            Self {
                coingecko_asset_platform_id: Default::default(),
                name: Default::default(),
            }
        }
    }
    ///`Ohlcv`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": {
    ///        "attributes": {
    ///          "ohlcv_list": [
    ///            [
    ///              1712534400,
    ///              3454.61590249189,
    ///              3660.85954963415,
    ///              3417.91885296256,
    ///              3660.85954963415,
    ///              306823.277031161
    ///            ],
    ///            [
    ///              1712448000,
    ///              3362.60273217873,
    ///              3455.28884490954,
    ///              3352.95305060685,
    ///              3454.61590249189,
    ///              242144.864784184
    ///            ],
    ///            [
    ///              1712361600,
    ///              3323.05578706056,
    ///              3391.19811016133,
    ///              3317.73497182435,
    ///              3362.60273217873,
    ///              273323.661682931
    ///            ]
    ///          ]
    ///        },
    ///        "id": "bc786a99-7205-4c80-aaa1-b9634d97c926",
    ///        "type": "ohlcv_request_response"
    ///      },
    ///      "meta": {
    ///        "base": {
    ///          "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///          "coingecko_coin_id": "weth",
    ///          "name": "Wrapped Ether",
    ///          "symbol": "WETH"
    ///        },
    ///        "quote": {
    ///          "address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    ///          "coingecko_coin_id": "tether",
    ///          "name": "Tether USD",
    ///          "symbol": "USDT"
    ///        }
    ///      }
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "object",
    ///      "properties": {
    ///        "attributes": {
    ///          "type": "object",
    ///          "properties": {
    ///            "ohlcv_list": {
    ///              "type": "array",
    ///              "items": {
    ///                "type": "array",
    ///                "items": {
    ///                  "type": "number"
    ///                }
    ///              }
    ///            }
    ///          }
    ///        },
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "type": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "meta": {
    ///      "type": "object",
    ///      "properties": {
    ///        "base": {
    ///          "type": "object",
    ///          "properties": {
    ///            "address": {
    ///              "type": "string"
    ///            },
    ///            "coingecko_coin_id": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "name": {
    ///              "type": "string"
    ///            },
    ///            "symbol": {
    ///              "type": "string"
    ///            }
    ///          }
    ///        },
    ///        "quote": {
    ///          "type": "object",
    ///          "properties": {
    ///            "address": {
    ///              "type": "string"
    ///            },
    ///            "coingecko_coin_id": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "name": {
    ///              "type": "string"
    ///            },
    ///            "symbol": {
    ///              "type": "string"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Ohlcv {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub data: ::std::option::Option<OhlcvData>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub meta: ::std::option::Option<OhlcvMeta>,
    }
    impl ::std::default::Default for Ohlcv {
        fn default() -> Self {
            Self {
                data: Default::default(),
                meta: Default::default(),
            }
        }
    }
    ///`OhlcvData`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "attributes": {
    ///      "type": "object",
    ///      "properties": {
    ///        "ohlcv_list": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "array",
    ///            "items": {
    ///              "type": "number"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "id": {
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
    pub struct OhlcvData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub attributes: ::std::option::Option<OhlcvDataAttributes>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for OhlcvData {
        fn default() -> Self {
            Self {
                attributes: Default::default(),
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`OhlcvDataAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "ohlcv_list": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "array",
    ///        "items": {
    ///          "type": "number"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct OhlcvDataAttributes {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub ohlcv_list: ::std::vec::Vec<::std::vec::Vec<f64>>,
    }
    impl ::std::default::Default for OhlcvDataAttributes {
        fn default() -> Self {
            Self {
                ohlcv_list: Default::default(),
            }
        }
    }
    ///`OhlcvMeta`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "base": {
    ///      "type": "object",
    ///      "properties": {
    ///        "address": {
    ///          "type": "string"
    ///        },
    ///        "coingecko_coin_id": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        },
    ///        "symbol": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "quote": {
    ///      "type": "object",
    ///      "properties": {
    ///        "address": {
    ///          "type": "string"
    ///        },
    ///        "coingecko_coin_id": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
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
    pub struct OhlcvMeta {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base: ::std::option::Option<OhlcvMetaBase>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote: ::std::option::Option<OhlcvMetaQuote>,
    }
    impl ::std::default::Default for OhlcvMeta {
        fn default() -> Self {
            Self {
                base: Default::default(),
                quote: Default::default(),
            }
        }
    }
    ///`OhlcvMetaBase`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "address": {
    ///      "type": "string"
    ///    },
    ///    "coingecko_coin_id": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
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
    pub struct OhlcvMetaBase {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub coingecko_coin_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub symbol: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for OhlcvMetaBase {
        fn default() -> Self {
            Self {
                address: Default::default(),
                coingecko_coin_id: Default::default(),
                name: Default::default(),
                symbol: Default::default(),
            }
        }
    }
    ///`OhlcvMetaQuote`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "address": {
    ///      "type": "string"
    ///    },
    ///    "coingecko_coin_id": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
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
    pub struct OhlcvMetaQuote {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub coingecko_coin_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub symbol: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for OhlcvMetaQuote {
        fn default() -> Self {
            Self {
                address: Default::default(),
                coingecko_coin_id: Default::default(),
                name: Default::default(),
                symbol: Default::default(),
            }
        }
    }
    ///`Pool`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": [
    ///        {
    ///          "attributes": {
    ///            "address": "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
    ///            "base_token_price_native_currency": "1.0",
    ///            "base_token_price_quote_token": "3662.46",
    ///            "base_token_price_usd": "3653.12491645176",
    ///            "fdv_usd": "11007041041",
    ///            "market_cap_usd": "11007041041",
    ///            "name": "WETH / USDC 0.05%",
    ///            "pool_created_at": "2021-12-29T12:35:14Z",
    ///            "price_change_percentage": {
    ///              "h1": "0.51",
    ///              "h24": "7.71",
    ///              "h6": "0.86",
    ///              "m15": "0.21",
    ///              "m30": "0.31",
    ///              "m5": "0"
    ///            },
    ///            "quote_token_price_base_token": "0.00027304",
    ///            "quote_token_price_native_currency": "0.000273040545093221",
    ///            "quote_token_price_usd": "0.998343707926245",
    ///            "reserve_in_usd": "163988541.3812",
    ///            "transactions": {
    ///              "h1": {
    ///                "buyers": 83,
    ///                "buys": 97,
    ///                "sellers": 124,
    ///                "sells": 144
    ///              },
    ///              "h24": {
    ///                "buyers": 1625,
    ///                "buys": 2966,
    ///                "sellers": 2399,
    ///                "sells": 3847
    ///              },
    ///              "m15": {
    ///                "buyers": 19,
    ///                "buys": 19,
    ///                "sellers": 27,
    ///                "sells": 27
    ///              },
    ///              "m30": {
    ///                "buyers": 45,
    ///                "buys": 49,
    ///                "sellers": 57,
    ///                "sells": 61
    ///              },
    ///              "m5": {
    ///                "buyers": 7,
    ///                "buys": 7,
    ///                "sellers": 2,
    ///                "sells": 2
    ///              }
    ///            },
    ///            "volume_usd": {
    ///              "h1": "16798158.0138526",
    ///              "h24": "536545444.904535",
    ///              "h6": "164054610.850188",
    ///              "m15": "2056262.885098",
    ///              "m30": "4081230.098456",
    ///              "m5": "868581.7348314"
    ///            }
    ///          },
    ///          "id": "eth_0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
    ///          "relationships": {
    ///            "base_token": {
    ///              "data": {
    ///                "id": "eth_0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///                "type": "token"
    ///              }
    ///            },
    ///            "dex": {
    ///              "data": {
    ///                "id": "uniswap_v3",
    ///                "type": "dex"
    ///              }
    ///            },
    ///            "quote_token": {
    ///              "data": {
    ///                "id": "eth_0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    ///                "type": "token"
    ///              }
    ///            }
    ///          },
    ///          "type": "pool"
    ///        }
    ///      ],
    ///      "included": [
    ///        {
    ///          "attributes": {
    ///            "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///            "coingecko_coin_id": "weth",
    ///            "decimals": 18,
    ///            "image_url": "https://assets.coingecko.com/coins/images/2518/small/weth.png?1696503332",
    ///            "name": "Wrapped Ether",
    ///            "symbol": "WETH"
    ///          },
    ///          "id": "eth_0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///          "type": "token"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/pool_resource"
    ///      }
    ///    },
    ///    "included": {
    ///      "$ref": "#/components/schemas/pools_included"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Pool {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<PoolResource>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub included: ::std::option::Option<PoolsIncluded>,
    }
    impl ::std::default::Default for Pool {
        fn default() -> Self {
            Self {
                data: Default::default(),
                included: Default::default(),
            }
        }
    }
    ///`PoolDetails`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "attributes": {
    ///      "type": "object",
    ///      "properties": {
    ///        "address": {
    ///          "type": "string"
    ///        },
    ///        "base_token_balance": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "base_token_liquidity_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "base_token_price_native_currency": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "base_token_price_quote_token": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "base_token_price_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "buy_volume_usd": {
    ///          "type": "object"
    ///        },
    ///        "fdv_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "launchpad_details": {
    ///          "type": [
    ///            "object",
    ///            "null"
    ///          ]
    ///        },
    ///        "locked_liquidity_percentage": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "market_cap_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        },
    ///        "net_buy_volume_usd": {
    ///          "type": "object"
    ///        },
    ///        "pool_created_at": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "pool_fee_percentage": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "pool_name": {
    ///          "type": "string"
    ///        },
    ///        "price_change_percentage": {
    ///          "type": "object"
    ///        },
    ///        "quote_token_balance": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "quote_token_liquidity_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "quote_token_price_base_token": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "quote_token_price_native_currency": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "quote_token_price_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "reserve_in_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "sell_volume_usd": {
    ///          "type": "object"
    ///        },
    ///        "transactions": {
    ///          "type": "object"
    ///        },
    ///        "volume_usd": {
    ///          "type": "object"
    ///        }
    ///      }
    ///    },
    ///    "id": {
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
    pub struct PoolDetails {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub attributes: ::std::option::Option<PoolDetailsAttributes>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PoolDetails {
        fn default() -> Self {
            Self {
                attributes: Default::default(),
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`PoolDetailsAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "address": {
    ///      "type": "string"
    ///    },
    ///    "base_token_balance": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "base_token_liquidity_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "base_token_price_native_currency": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "base_token_price_quote_token": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "base_token_price_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "buy_volume_usd": {
    ///      "type": "object"
    ///    },
    ///    "fdv_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "launchpad_details": {
    ///      "type": [
    ///        "object",
    ///        "null"
    ///      ]
    ///    },
    ///    "locked_liquidity_percentage": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "market_cap_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "net_buy_volume_usd": {
    ///      "type": "object"
    ///    },
    ///    "pool_created_at": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "pool_fee_percentage": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "pool_name": {
    ///      "type": "string"
    ///    },
    ///    "price_change_percentage": {
    ///      "type": "object"
    ///    },
    ///    "quote_token_balance": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "quote_token_liquidity_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "quote_token_price_base_token": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "quote_token_price_native_currency": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "quote_token_price_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "reserve_in_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "sell_volume_usd": {
    ///      "type": "object"
    ///    },
    ///    "transactions": {
    ///      "type": "object"
    ///    },
    ///    "volume_usd": {
    ///      "type": "object"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PoolDetailsAttributes {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base_token_balance: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base_token_liquidity_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base_token_price_native_currency: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base_token_price_quote_token: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base_token_price_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub buy_volume_usd: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub fdv_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub launchpad_details:
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub locked_liquidity_percentage: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub market_cap_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub net_buy_volume_usd: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pool_created_at: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pool_fee_percentage: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pool_name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub price_change_percentage: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote_token_balance: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote_token_liquidity_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote_token_price_base_token: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote_token_price_native_currency: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote_token_price_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub reserve_in_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub sell_volume_usd: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub transactions: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub volume_usd: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    }
    impl ::std::default::Default for PoolDetailsAttributes {
        fn default() -> Self {
            Self {
                address: Default::default(),
                base_token_balance: Default::default(),
                base_token_liquidity_usd: Default::default(),
                base_token_price_native_currency: Default::default(),
                base_token_price_quote_token: Default::default(),
                base_token_price_usd: Default::default(),
                buy_volume_usd: Default::default(),
                fdv_usd: Default::default(),
                launchpad_details: Default::default(),
                locked_liquidity_percentage: Default::default(),
                market_cap_usd: Default::default(),
                name: Default::default(),
                net_buy_volume_usd: Default::default(),
                pool_created_at: Default::default(),
                pool_fee_percentage: Default::default(),
                pool_name: Default::default(),
                price_change_percentage: Default::default(),
                quote_token_balance: Default::default(),
                quote_token_liquidity_usd: Default::default(),
                quote_token_price_base_token: Default::default(),
                quote_token_price_native_currency: Default::default(),
                quote_token_price_usd: Default::default(),
                reserve_in_usd: Default::default(),
                sell_volume_usd: Default::default(),
                transactions: Default::default(),
                volume_usd: Default::default(),
            }
        }
    }
    ///`PoolResource`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "attributes": {
    ///      "type": "object",
    ///      "properties": {
    ///        "address": {
    ///          "type": "string"
    ///        },
    ///        "base_token_price_native_currency": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "base_token_price_quote_token": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "base_token_price_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "fdv_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "market_cap_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        },
    ///        "pool_created_at": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "price_change_percentage": {
    ///          "type": "object"
    ///        },
    ///        "quote_token_price_base_token": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "quote_token_price_native_currency": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "quote_token_price_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "reserve_in_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "transactions": {
    ///          "type": "object"
    ///        },
    ///        "volume_usd": {
    ///          "type": "object"
    ///        }
    ///      }
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "relationships": {
    ///      "type": "object",
    ///      "properties": {
    ///        "base_token": {
    ///          "type": "object",
    ///          "properties": {
    ///            "data": {
    ///              "type": "object",
    ///              "properties": {
    ///                "id": {
    ///                  "type": "string"
    ///                },
    ///                "type": {
    ///                  "type": "string"
    ///                }
    ///              }
    ///            }
    ///          }
    ///        },
    ///        "dex": {
    ///          "type": "object",
    ///          "properties": {
    ///            "data": {
    ///              "type": "object",
    ///              "properties": {
    ///                "id": {
    ///                  "type": "string"
    ///                },
    ///                "type": {
    ///                  "type": "string"
    ///                }
    ///              }
    ///            }
    ///          }
    ///        },
    ///        "quote_token": {
    ///          "type": "object",
    ///          "properties": {
    ///            "data": {
    ///              "type": "object",
    ///              "properties": {
    ///                "id": {
    ///                  "type": "string"
    ///                },
    ///                "type": {
    ///                  "type": "string"
    ///                }
    ///              }
    ///            }
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PoolResource {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub attributes: ::std::option::Option<PoolResourceAttributes>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub relationships: ::std::option::Option<PoolResourceRelationships>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PoolResource {
        fn default() -> Self {
            Self {
                attributes: Default::default(),
                id: Default::default(),
                relationships: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`PoolResourceAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "address": {
    ///      "type": "string"
    ///    },
    ///    "base_token_price_native_currency": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "base_token_price_quote_token": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "base_token_price_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "fdv_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "market_cap_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "pool_created_at": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "price_change_percentage": {
    ///      "type": "object"
    ///    },
    ///    "quote_token_price_base_token": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "quote_token_price_native_currency": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "quote_token_price_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "reserve_in_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "transactions": {
    ///      "type": "object"
    ///    },
    ///    "volume_usd": {
    ///      "type": "object"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PoolResourceAttributes {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base_token_price_native_currency: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base_token_price_quote_token: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base_token_price_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub fdv_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub market_cap_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub pool_created_at: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub price_change_percentage: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote_token_price_base_token: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote_token_price_native_currency: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote_token_price_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub reserve_in_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub transactions: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub volume_usd: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
    }
    impl ::std::default::Default for PoolResourceAttributes {
        fn default() -> Self {
            Self {
                address: Default::default(),
                base_token_price_native_currency: Default::default(),
                base_token_price_quote_token: Default::default(),
                base_token_price_usd: Default::default(),
                fdv_usd: Default::default(),
                market_cap_usd: Default::default(),
                name: Default::default(),
                pool_created_at: Default::default(),
                price_change_percentage: Default::default(),
                quote_token_price_base_token: Default::default(),
                quote_token_price_native_currency: Default::default(),
                quote_token_price_usd: Default::default(),
                reserve_in_usd: Default::default(),
                transactions: Default::default(),
                volume_usd: Default::default(),
            }
        }
    }
    ///`PoolResourceRelationships`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "base_token": {
    ///      "type": "object",
    ///      "properties": {
    ///        "data": {
    ///          "type": "object",
    ///          "properties": {
    ///            "id": {
    ///              "type": "string"
    ///            },
    ///            "type": {
    ///              "type": "string"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "dex": {
    ///      "type": "object",
    ///      "properties": {
    ///        "data": {
    ///          "type": "object",
    ///          "properties": {
    ///            "id": {
    ///              "type": "string"
    ///            },
    ///            "type": {
    ///              "type": "string"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "quote_token": {
    ///      "type": "object",
    ///      "properties": {
    ///        "data": {
    ///          "type": "object",
    ///          "properties": {
    ///            "id": {
    ///              "type": "string"
    ///            },
    ///            "type": {
    ///              "type": "string"
    ///            }
    ///          }
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PoolResourceRelationships {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub base_token: ::std::option::Option<PoolResourceRelationshipsBaseToken>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub dex: ::std::option::Option<PoolResourceRelationshipsDex>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quote_token: ::std::option::Option<PoolResourceRelationshipsQuoteToken>,
    }
    impl ::std::default::Default for PoolResourceRelationships {
        fn default() -> Self {
            Self {
                base_token: Default::default(),
                dex: Default::default(),
                quote_token: Default::default(),
            }
        }
    }
    ///`PoolResourceRelationshipsBaseToken`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "object",
    ///      "properties": {
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "type": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PoolResourceRelationshipsBaseToken {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub data: ::std::option::Option<PoolResourceRelationshipsBaseTokenData>,
    }
    impl ::std::default::Default for PoolResourceRelationshipsBaseToken {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`PoolResourceRelationshipsBaseTokenData`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
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
    pub struct PoolResourceRelationshipsBaseTokenData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PoolResourceRelationshipsBaseTokenData {
        fn default() -> Self {
            Self {
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`PoolResourceRelationshipsDex`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "object",
    ///      "properties": {
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "type": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PoolResourceRelationshipsDex {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub data: ::std::option::Option<PoolResourceRelationshipsDexData>,
    }
    impl ::std::default::Default for PoolResourceRelationshipsDex {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`PoolResourceRelationshipsDexData`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
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
    pub struct PoolResourceRelationshipsDexData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PoolResourceRelationshipsDexData {
        fn default() -> Self {
            Self {
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`PoolResourceRelationshipsQuoteToken`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "object",
    ///      "properties": {
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "type": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PoolResourceRelationshipsQuoteToken {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub data: ::std::option::Option<PoolResourceRelationshipsQuoteTokenData>,
    }
    impl ::std::default::Default for PoolResourceRelationshipsQuoteToken {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`PoolResourceRelationshipsQuoteTokenData`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
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
    pub struct PoolResourceRelationshipsQuoteTokenData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PoolResourceRelationshipsQuoteTokenData {
        fn default() -> Self {
            Self {
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`PoolTokensInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": [
    ///        {
    ///          "attributes": {
    ///            "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///            "banner_image_url": "https://assets.geckoterminal.com/vgvwashzmsg7qt9iamrclayfplde",
    ///            "categories": [],
    ///            "coingecko_coin_id": "weth",
    ///            "description": "WETH is the tokenized/packaged form of ETH that you use to pay for items when you interact with Ethereum dApps...",
    ///            "developer_address": null,
    ///            "developer_holding_percentage": null,
    ///            "discord_url": null,
    ///            "freeze_authority": null,
    ///            "gt_categories_id": [],
    ///            "gt_score": 92.6605504587156,
    ///            "gt_score_details": {
    ///              "creation": 100,
    ///              "holders": 100,
    ///              "info": 100,
    ///              "pool": 87.5,
    ///              "transaction": 0
    ///            },
    ///            "holders": {
    ///              "count": 1385496,
    ///              "distribution_percentage": {
    ///                "11_30": "13.5825",
    ///                "31_50": "4.7971",
    ///                "rest": "26.502",
    ///                "top_10": "55.1184"
    ///              },
    ///              "last_updated": "2025-03-12T13:07:47Z"
    ///            },
    ///            "image": {
    ///              "large": "https://assets.coingecko.com/coins/images/2518/large/weth.png?1696503332",
    ///              "small": "https://assets.coingecko.com/coins/images/2518/small/weth.png?1696503332",
    ///              "thumb": "https://assets.coingecko.com/coins/images/2518/thumb/weth.png?1696503332"
    ///            },
    ///            "image_url": "https://assets.coingecko.com/coins/images/2518/small/weth.png?1696503332",
    ///            "is_honeypot": false,
    ///            "mint_authority": null,
    ///            "name": "Wrapped Ether",
    ///            "symbol": "WETH",
    ///            "telegram_handle": null,
    ///            "twitter_handle": null,
    ///            "websites": [
    ///              "https://weth.io/"
    ///            ]
    ///          },
    ///          "id": "eth_0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///          "type": "token"
    ///        },
    ///        {
    ///          "attributes": {
    ///            "address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    ///            "categories": [],
    ///            "coingecko_coin_id": "tether",
    ///            "description": "Tether (USDT) is a cryptocurrency with a value meant to mirror the value of the U.S. dollar. ...",
    ///            "developer_address": null,
    ///            "developer_holding_percentage": null,
    ///            "discord_url": null,
    ///            "freeze_authority": null,
    ///            "gt_categories_id": [],
    ///            "gt_score": 92.6605504587156,
    ///            "gt_score_details": {
    ///              "creation": 100,
    ///              "holders": 0,
    ///              "info": 100,
    ///              "pool": 87.5,
    ///              "transaction": 0
    ///            },
    ///            "holders": {
    ///              "count": 7041203,
    ///              "distribution_percentage": {
    ///                "11_30": "13.4293",
    ///                "31_50": "3.9681",
    ///                "rest": "37.0244",
    ///                "top_10": "45.5782"
    ///              },
    ///              "last_updated": "2025-03-12T05:28:50Z"
    ///            },
    ///            "image_url": "https://assets.coingecko.com/coins/images/325/small/Tether.png?1696501661",
    ///            "is_honeypot": false,
    ///            "mint_authority": null,
    ///            "name": "Tether USD",
    ///            "symbol": "USDT",
    ///            "telegram_handle": null,
    ///            "twitter_handle": "Tether_to",
    ///            "websites": [
    ///              "https://tether.to/"
    ///            ]
    ///          },
    ///          "id": "eth_0xdac17f958d2ee523a2206206994597c13d831ec7",
    ///          "type": "token"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/token_info"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct PoolTokensInfo {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<TokenInfo>,
    }
    impl ::std::default::Default for PoolTokensInfo {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`PoolsIncluded`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "array",
    ///  "items": {
    ///    "type": "object",
    ///    "properties": {
    ///      "attributes": {
    ///        "type": "object",
    ///        "properties": {
    ///          "address": {
    ///            "type": "string"
    ///          },
    ///          "coingecko_coin_id": {
    ///            "type": [
    ///              "string",
    ///              "null"
    ///            ]
    ///          },
    ///          "decimals": {
    ///            "type": [
    ///              "integer",
    ///              "null"
    ///            ]
    ///          },
    ///          "name": {
    ///            "type": "string"
    ///          },
    ///          "symbol": {
    ///            "type": [
    ///              "string",
    ///              "null"
    ///            ]
    ///          }
    ///        }
    ///      },
    ///      "id": {
    ///        "type": "string"
    ///      },
    ///      "type": {
    ///        "type": "string"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct PoolsIncluded(pub ::std::vec::Vec<PoolsIncludedItem>);
    impl ::std::ops::Deref for PoolsIncluded {
        type Target = ::std::vec::Vec<PoolsIncludedItem>;
        fn deref(&self) -> &::std::vec::Vec<PoolsIncludedItem> {
            &self.0
        }
    }
    impl ::std::convert::From<PoolsIncluded> for ::std::vec::Vec<PoolsIncludedItem> {
        fn from(value: PoolsIncluded) -> Self {
            value.0
        }
    }
    impl ::std::convert::From<::std::vec::Vec<PoolsIncludedItem>> for PoolsIncluded {
        fn from(value: ::std::vec::Vec<PoolsIncludedItem>) -> Self {
            Self(value)
        }
    }
    ///`PoolsIncludedItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "attributes": {
    ///      "type": "object",
    ///      "properties": {
    ///        "address": {
    ///          "type": "string"
    ///        },
    ///        "coingecko_coin_id": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "decimals": {
    ///          "type": [
    ///            "integer",
    ///            "null"
    ///          ]
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        },
    ///        "symbol": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    "id": {
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
    pub struct PoolsIncludedItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub attributes: ::std::option::Option<PoolsIncludedItemAttributes>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PoolsIncludedItem {
        fn default() -> Self {
            Self {
                attributes: Default::default(),
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`PoolsIncludedItemAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "address": {
    ///      "type": "string"
    ///    },
    ///    "coingecko_coin_id": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "decimals": {
    ///      "type": [
    ///        "integer",
    ///        "null"
    ///      ]
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "symbol": {
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
    pub struct PoolsIncludedItemAttributes {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub coingecko_coin_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub decimals: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub symbol: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for PoolsIncludedItemAttributes {
        fn default() -> Self {
            Self {
                address: Default::default(),
                coingecko_coin_id: Default::default(),
                decimals: Default::default(),
                name: Default::default(),
                symbol: Default::default(),
            }
        }
    }
    ///`Relationships`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object"
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(transparent)]
    pub struct Relationships(pub ::serde_json::Map<::std::string::String, ::serde_json::Value>);
    impl ::std::ops::Deref for Relationships {
        type Target = ::serde_json::Map<::std::string::String, ::serde_json::Value>;
        fn deref(&self) -> &::serde_json::Map<::std::string::String, ::serde_json::Value> {
            &self.0
        }
    }
    impl ::std::convert::From<Relationships>
        for ::serde_json::Map<::std::string::String, ::serde_json::Value>
    {
        fn from(value: Relationships) -> Self {
            value.0
        }
    }
    impl ::std::convert::From<::serde_json::Map<::std::string::String, ::serde_json::Value>>
        for Relationships
    {
        fn from(value: ::serde_json::Map<::std::string::String, ::serde_json::Value>) -> Self {
            Self(value)
        }
    }
    ///`SimpleTokenPrice`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": {
    ///        "attributes": {
    ///          "h24_price_change_percentage": {
    ///            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "3.3870290336"
    ///          },
    ///          "h24_volume_usd": {
    ///            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "965988358.733808"
    ///          },
    ///          "market_cap_usd": {
    ///            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "6692452895.779648"
    ///          },
    ///          "token_prices": {
    ///            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "2289.33"
    ///          },
    ///          "total_reserve_in_usd": {
    ///            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1576179559.94669772339136684208"
    ///          }
    ///        },
    ///        "id": "1ba898f0-eda2-4291-9491-9a5b323f66ef",
    ///        "type": "simple_token_price"
    ///      }
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "object",
    ///      "properties": {
    ///        "attributes": {
    ///          "type": "object",
    ///          "properties": {
    ///            "token_prices": {
    ///              "type": "object",
    ///              "additionalProperties": {
    ///                "type": "string"
    ///              }
    ///            }
    ///          }
    ///        },
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "type": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct SimpleTokenPrice {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub data: ::std::option::Option<SimpleTokenPriceData>,
    }
    impl ::std::default::Default for SimpleTokenPrice {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`SimpleTokenPriceData`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "attributes": {
    ///      "type": "object",
    ///      "properties": {
    ///        "token_prices": {
    ///          "type": "object",
    ///          "additionalProperties": {
    ///            "type": "string"
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "id": {
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
    pub struct SimpleTokenPriceData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub attributes: ::std::option::Option<SimpleTokenPriceDataAttributes>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for SimpleTokenPriceData {
        fn default() -> Self {
            Self {
                attributes: Default::default(),
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`SimpleTokenPriceDataAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "token_prices": {
    ///      "type": "object",
    ///      "additionalProperties": {
    ///        "type": "string"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct SimpleTokenPriceDataAttributes {
        #[serde(
            default,
            skip_serializing_if = ":: std :: collections :: HashMap::is_empty"
        )]
        pub token_prices: ::std::collections::HashMap<::std::string::String, ::std::string::String>,
    }
    impl ::std::default::Default for SimpleTokenPriceDataAttributes {
        fn default() -> Self {
            Self {
                token_prices: Default::default(),
            }
        }
    }
    ///`SinglePoolDetail`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": {
    ///        "attributes": {
    ///          "address": "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
    ///          "base_token_balance": "5713.28147604894",
    ///          "base_token_liquidity_usd": "25559474.76756355",
    ///          "base_token_price_native_currency": "1.0",
    ///          "base_token_price_quote_token": "4483.543371561",
    ///          "base_token_price_usd": "4473.69429892668",
    ///          "buy_volume_usd": {
    ///            "h1": "305597.110959759",
    ///            "h24": "7260323.6767418",
    ///            "h6": "1331928.64807714",
    ///            "m15": "58659.5847657773",
    ///            "m30": "164918.943968904",
    ///            "m5": "0.0"
    ///          },
    ///          "fdv_usd": "11190873336.847",
    ///          "locked_liquidity_percentage": "0.0",
    ///          "market_cap_usd": "11195941219.2312",
    ///          "name": "WETH / USDC 0.05%",
    ///          "net_buy_volume_usd": {
    ///            "h1": "213181.8440453441",
    ///            "h24": "-3562.19491886",
    ///            "h6": "39055.74647786",
    ///            "m15": "20088.5420905036",
    ///            "m30": "112265.1687880188",
    ///            "m5": "-14474.0609971508"
    ///          },
    ///          "pool_created_at": "2021-12-29T12:35:14Z",
    ///          "pool_fee_percentage": "0.05",
    ///          "pool_name": "WETH / USDC",
    ///          "price_change_percentage": {
    ///            "h1": "0",
    ///            "h24": "0.06",
    ///            "h6": "-0.31",
    ///            "m15": "0.01",
    ///            "m30": "0",
    ///            "m5": "0"
    ///          },
    ///          "quote_token_balance": "64636039.082111",
    ///          "quote_token_liquidity_usd": "64494052.04397222",
    ///          "quote_token_price_base_token": "0.0002230378781",
    ///          "quote_token_price_native_currency": "0.000223037878108427",
    ///          "quote_token_price_usd": "0.997996963545633",
    ///          "reserve_in_usd": "90053526.8909",
    ///          "sell_volume_usd": {
    ///            "h1": "92415.2669144149",
    ///            "h24": "7263885.87166066",
    ///            "h6": "1292872.90159928",
    ///            "m15": "38571.0426752737",
    ///            "m30": "52653.7751808852",
    ///            "m5": "14474.0609971508"
    ///          },
    ///          "transactions": {
    ///            "h1": {
    ///              "buyers": 52,
    ///              "buys": 72,
    ///              "sellers": 55,
    ///              "sells": 66
    ///            },
    ///            "h24": {
    ///              "buyers": 720,
    ///              "buys": 1405,
    ///              "sellers": 1312,
    ///              "sells": 1769
    ///            },
    ///            "h6": {
    ///              "buyers": 270,
    ///              "buys": 394,
    ///              "sellers": 306,
    ///              "sells": 374
    ///            },
    ///            "m15": {
    ///              "buyers": 7,
    ///              "buys": 7,
    ///              "sellers": 13,
    ///              "sells": 14
    ///            },
    ///            "m30": {
    ///              "buyers": 30,
    ///              "buys": 36,
    ///              "sellers": 31,
    ///              "sells": 36
    ///            },
    ///            "m5": {
    ///              "buyers": 0,
    ///              "buys": 0,
    ///              "sellers": 4,
    ///              "sells": 4
    ///            }
    ///          },
    ///          "volume_usd": {
    ///            "h1": "398012.377874174",
    ///            "h24": "14524209.5484025",
    ///            "h6": "2624801.54967643",
    ///            "m15": "97230.627441051",
    ///            "m30": "217572.719149789",
    ///            "m5": "14474.0609971508"
    ///          }
    ///        },
    ///        "id": "eth_0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640",
    ///        "relationships": {
    ///          "base_token": {
    ///            "data": {
    ///              "id": "eth_0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///              "type": "token"
    ///            }
    ///          },
    ///          "dex": {
    ///            "data": {
    ///              "id": "uniswap_v3",
    ///              "type": "dex"
    ///            }
    ///          },
    ///          "quote_token": {
    ///            "data": {
    ///              "id": "eth_0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    ///              "type": "token"
    ///            }
    ///          }
    ///        },
    ///        "type": "pool"
    ///      },
    ///      "included": [
    ///        {
    ///          "attributes": {
    ///            "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///            "coingecko_coin_id": "weth",
    ///            "decimals": 18,
    ///            "image_url": "https://coin-images.coingecko.com/coins/images/2518/large/weth.png?1696503332",
    ///            "name": "Wrapped Ether",
    ///            "symbol": "WETH"
    ///          },
    ///          "id": "eth_0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///          "type": "token"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "$ref": "#/components/schemas/pool_details"
    ///    },
    ///    "included": {
    ///      "$ref": "#/components/schemas/pools_included"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct SinglePoolDetail {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub data: ::std::option::Option<PoolDetails>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub included: ::std::option::Option<PoolsIncluded>,
    }
    impl ::std::default::Default for SinglePoolDetail {
        fn default() -> Self {
            Self {
                data: Default::default(),
                included: Default::default(),
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
    ///    "attributes",
    ///    "id",
    ///    "type"
    ///  ],
    ///  "properties": {
    ///    "attributes": {
    ///      "type": "object",
    ///      "properties": {
    ///        "address": {
    ///          "type": "string"
    ///        },
    ///        "coingecko_coin_id": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "decimals": {
    ///          "type": "integer"
    ///        },
    ///        "fdv_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "market_cap_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        },
    ///        "normalized_total_supply": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "price_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "symbol": {
    ///          "type": "string"
    ///        },
    ///        "total_reserve_in_usd": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "total_supply": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "volume_usd": {
    ///          "type": [
    ///            "object",
    ///            "null"
    ///          ]
    ///        }
    ///      }
    ///    },
    ///    "id": {
    ///      "type": "string"
    ///    },
    ///    "relationships": {
    ///      "type": "object",
    ///      "properties": {
    ///        "top_pools": {
    ///          "type": "object",
    ///          "properties": {
    ///            "data": {
    ///              "type": "array",
    ///              "items": {
    ///                "type": "object",
    ///                "properties": {
    ///                  "id": {
    ///                    "type": "string"
    ///                  },
    ///                  "type": {
    ///                    "type": "string"
    ///                  }
    ///                }
    ///              }
    ///            }
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "type": {
    ///      "type": "string",
    ///      "enum": [
    ///        "token"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Token {
        pub attributes: TokenAttributes,
        pub id: ::std::string::String,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub relationships: ::std::option::Option<TokenRelationships>,
        #[serde(rename = "type")]
        pub type_: TokenType,
    }
    ///`TokenAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "address": {
    ///      "type": "string"
    ///    },
    ///    "coingecko_coin_id": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "decimals": {
    ///      "type": "integer"
    ///    },
    ///    "fdv_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "market_cap_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "normalized_total_supply": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "price_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "symbol": {
    ///      "type": "string"
    ///    },
    ///    "total_reserve_in_usd": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "total_supply": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "volume_usd": {
    ///      "type": [
    ///        "object",
    ///        "null"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct TokenAttributes {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub coingecko_coin_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub decimals: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub fdv_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub market_cap_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub normalized_total_supply: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub symbol: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_reserve_in_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub total_supply: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub volume_usd:
            ::std::option::Option<::serde_json::Map<::std::string::String, ::serde_json::Value>>,
    }
    impl ::std::default::Default for TokenAttributes {
        fn default() -> Self {
            Self {
                address: Default::default(),
                coingecko_coin_id: Default::default(),
                decimals: Default::default(),
                fdv_usd: Default::default(),
                market_cap_usd: Default::default(),
                name: Default::default(),
                normalized_total_supply: Default::default(),
                price_usd: Default::default(),
                symbol: Default::default(),
                total_reserve_in_usd: Default::default(),
                total_supply: Default::default(),
                volume_usd: Default::default(),
            }
        }
    }
    ///`TokenDetail`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": {
    ///        "attributes": {
    ///          "address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    ///          "banner_image_url": null,
    ///          "coingecko_coin_id": "tether",
    ///          "decimals": 6,
    ///          "fdv_usd": "91700939859.6687",
    ///          "image_url": "https://coin-images.coingecko.com/coins/images/325/large/Tether.png?1696501661",
    ///          "market_cap_usd": "171798403974.784",
    ///          "name": "Tether USD",
    ///          "normalized_total_supply": "91775654692.2505",
    ///          "price_usd": "0.999188255",
    ///          "symbol": "USDT",
    ///          "total_reserve_in_usd": "405089394.14192413773442554227",
    ///          "total_supply": "91775654692250534.0",
    ///          "volume_usd": {
    ///            "h24": "1142454033.37436"
    ///          }
    ///        },
    ///        "id": "eth_0xdac17f958d2ee523a2206206994597c13d831ec7",
    ///        "relationships": {
    ///          "top_pools": {
    ///            "data": [
    ///              {
    ///                "id": "eth_0x667701e51b4d1ca244f17c78f7ab8744b4c99f9b",
    ///                "type": "pool"
    ///              },
    ///              {
    ///                "id": "eth_0xf063bd202e45d6b2843102cb4ece339026645d4a",
    ///                "type": "pool"
    ///              },
    ///              {
    ///                "id": "eth_0x8aa4e11cbdf30eedc92100f4c8a31ff748e201d44712cc8c90d189edaa8e4e47",
    ///                "type": "pool"
    ///              }
    ///            ]
    ///          }
    ///        },
    ///        "type": "token"
    ///      },
    ///      "included": [
    ///        {
    ///          "attributes": {
    ///            "address": "0x667701e51b4d1ca244f17c78f7ab8744b4c99f9b",
    ///            "base_token_balance": "74198606.301554",
    ///            "base_token_liquidity_usd": "74194892.50738515",
    ///            "base_token_price_native_currency": "0.000223341206041575",
    ///            "base_token_price_quote_token": "1.0007623117",
    ///            "base_token_price_usd": "0.999949947925521",
    ///            "fdv_usd": "91700939859.6687",
    ///            "market_cap_usd": "171798403975.064",
    ///            "name": "USDC / USDT",
    ///            "pool_created_at": "2025-09-18T08:28:30Z",
    ///            "price_change_percentage": {
    ///              "h1": "0",
    ///              "h24": "-0.5",
    ///              "h6": "-0.18",
    ///              "m15": "-0.65",
    ///              "m30": "-0.01",
    ///              "m5": "-0.02"
    ///            },
    ///            "quote_token_balance": "29771050.29628",
    ///            "quote_token_liquidity_usd": "29746883.795105774",
    ///            "quote_token_price_base_token": "0.9992382689",
    ///            "quote_token_price_native_currency": "0.000223171080110164",
    ///            "quote_token_price_usd": "0.99918825500163",
    ///            "reserve_in_usd": "103939687.035",
    ///            "token_price_usd": "0.99918825500163",
    ///            "transactions": {
    ///              "h1": {
    ///                "buyers": 16,
    ///                "buys": 39,
    ///                "sellers": 15,
    ///                "sells": 48
    ///              },
    ///              "h24": {
    ///                "buyers": 55,
    ///                "buys": 870,
    ///                "sellers": 59,
    ///                "sells": 1109
    ///              },
    ///              "h6": {
    ///                "buyers": 31,
    ///                "buys": 180,
    ///                "sellers": 35,
    ///                "sells": 271
    ///              },
    ///              "m15": {
    ///                "buyers": 3,
    ///                "buys": 4,
    ///                "sellers": 5,
    ///                "sells": 7
    ///              },
    ///              "m30": {
    ///                "buyers": 7,
    ///                "buys": 12,
    ///                "sellers": 9,
    ///                "sells": 19
    ///              },
    ///              "m5": {
    ///                "buyers": 0,
    ///                "buys": 0,
    ///                "sellers": 2,
    ///                "sells": 2
    ///              }
    ///            },
    ///            "volume_usd": {
    ///              "h1": "9434171.29930675",
    ///              "h24": "388004708.962186",
    ///              "h6": "84784863.3965608",
    ///              "m15": "1857835.95996426",
    ///              "m30": "3190909.49729904",
    ///              "m5": "988.7572917698"
    ///            }
    ///          },
    ///          "id": "eth_0x667701e51b4d1ca244f17c78f7ab8744b4c99f9b",
    ///          "relationships": {
    ///            "base_token": {
    ///              "data": {
    ///                "id": "eth_0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    ///                "type": "token"
    ///              }
    ///            },
    ///            "dex": {
    ///              "data": {
    ///                "id": "fluid-ethereum",
    ///                "type": "dex"
    ///              }
    ///            },
    ///            "quote_token": {
    ///              "data": {
    ///                "id": "eth_0xdac17f958d2ee523a2206206994597c13d831ec7",
    ///                "type": "token"
    ///              }
    ///            }
    ///          },
    ///          "type": "pool"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "$ref": "#/components/schemas/token"
    ///    },
    ///    "included": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/pool"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct TokenDetail {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub data: ::std::option::Option<Token>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub included: ::std::vec::Vec<Pool>,
    }
    impl ::std::default::Default for TokenDetail {
        fn default() -> Self {
            Self {
                data: Default::default(),
                included: Default::default(),
            }
        }
    }
    ///`TokenInfo`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": {
    ///        "attributes": {
    ///          "address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    ///          "banner_image_url": "https://assets.geckoterminal.com/vgvwashzmsg7qt9iamrclayfplde",
    ///          "categories": [],
    ///          "coingecko_coin_id": "tether",
    ///          "description": "Tether (USDT) is a cryptocurrency with a value meant to mirror the value of the U.S. dollar...",
    ///          "developer_address": null,
    ///          "developer_holding_percentage": null,
    ///          "discord_url": null,
    ///          "freeze_authority": null,
    ///          "gt_categories_id": [],
    ///          "gt_score": 92.6605504587156,
    ///          "gt_score_details": {
    ///            "creation": 100,
    ///            "holders": 0,
    ///            "info": 100,
    ///            "pool": 87.5,
    ///            "transaction": 0
    ///          },
    ///          "holders": {
    ///            "count": 7041203,
    ///            "distribution_percentage": {
    ///              "11_30": "13.4293",
    ///              "31_50": "3.9681",
    ///              "rest": "37.0244",
    ///              "top_10": "45.5782"
    ///            },
    ///            "last_updated": "2025-03-12T05:28:50Z"
    ///          },
    ///          "image": {
    ///            "large": "https://assets.coingecko.com/coins/images/325/large/Tether.png?1696501661",
    ///            "small": "https://assets.coingecko.com/coins/images/325/small/Tether.png?1696501661",
    ///            "thumb": "https://assets.coingecko.com/coins/images/325/thumb/Tether.png?1696501661"
    ///          },
    ///          "image_url": "https://assets.coingecko.com/coins/images/325/small/Tether.png?1696501661",
    ///          "is_honeypot": false,
    ///          "mint_authority": null,
    ///          "name": "Tether USD",
    ///          "symbol": "USDT",
    ///          "telegram_handle": null,
    ///          "twitter_handle": "Tether_to",
    ///          "websites": [
    ///            "https://tether.to/"
    ///          ]
    ///        },
    ///        "id": "eth_0xdac17f958d2ee523a2206206994597c13d831ec7",
    ///        "type": "token"
    ///      }
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "object",
    ///      "properties": {
    ///        "attributes": {
    ///          "type": "object",
    ///          "properties": {
    ///            "address": {
    ///              "type": "string"
    ///            },
    ///            "categories": {
    ///              "type": [
    ///                "array",
    ///                "null"
    ///              ],
    ///              "items": {
    ///                "type": "string"
    ///              }
    ///            },
    ///            "coingecko_coin_id": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "description": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "developer_address": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "developer_holding_percentage": {
    ///              "type": [
    ///                "number",
    ///                "null"
    ///              ]
    ///            },
    ///            "discord_url": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "freeze_authority": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "gt_score": {
    ///              "type": [
    ///                "number",
    ///                "null"
    ///              ]
    ///            },
    ///            "gt_score_details": {
    ///              "type": "object"
    ///            },
    ///            "holders": {
    ///              "type": "object"
    ///            },
    ///            "is_honeypot": {
    ///              "oneOf": [
    ///                {
    ///                  "type": "boolean"
    ///                },
    ///                {
    ///                  "type": "string"
    ///                }
    ///              ]
    ///            },
    ///            "mint_authority": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "name": {
    ///              "type": "string"
    ///            },
    ///            "symbol": {
    ///              "type": "string"
    ///            },
    ///            "telegram_handle": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "twitter_handle": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "websites": {
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string"
    ///              }
    ///            }
    ///          }
    ///        },
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "type": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct TokenInfo {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub data: ::std::option::Option<TokenInfoData>,
    }
    impl ::std::default::Default for TokenInfo {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`TokenInfoData`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "attributes": {
    ///      "type": "object",
    ///      "properties": {
    ///        "address": {
    ///          "type": "string"
    ///        },
    ///        "categories": {
    ///          "type": [
    ///            "array",
    ///            "null"
    ///          ],
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        },
    ///        "coingecko_coin_id": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "description": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "developer_address": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "developer_holding_percentage": {
    ///          "type": [
    ///            "number",
    ///            "null"
    ///          ]
    ///        },
    ///        "discord_url": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "freeze_authority": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "gt_score": {
    ///          "type": [
    ///            "number",
    ///            "null"
    ///          ]
    ///        },
    ///        "gt_score_details": {
    ///          "type": "object"
    ///        },
    ///        "holders": {
    ///          "type": "object"
    ///        },
    ///        "is_honeypot": {
    ///          "oneOf": [
    ///            {
    ///              "type": "boolean"
    ///            },
    ///            {
    ///              "type": "string"
    ///            }
    ///          ]
    ///        },
    ///        "mint_authority": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "name": {
    ///          "type": "string"
    ///        },
    ///        "symbol": {
    ///          "type": "string"
    ///        },
    ///        "telegram_handle": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "twitter_handle": {
    ///          "type": [
    ///            "string",
    ///            "null"
    ///          ]
    ///        },
    ///        "websites": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "string"
    ///          }
    ///        }
    ///      }
    ///    },
    ///    "id": {
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
    pub struct TokenInfoData {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub attributes: ::std::option::Option<TokenInfoDataAttributes>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for TokenInfoData {
        fn default() -> Self {
            Self {
                attributes: Default::default(),
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`TokenInfoDataAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "address": {
    ///      "type": "string"
    ///    },
    ///    "categories": {
    ///      "type": [
    ///        "array",
    ///        "null"
    ///      ],
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "coingecko_coin_id": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "description": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "developer_address": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "developer_holding_percentage": {
    ///      "type": [
    ///        "number",
    ///        "null"
    ///      ]
    ///    },
    ///    "discord_url": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "freeze_authority": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "gt_score": {
    ///      "type": [
    ///        "number",
    ///        "null"
    ///      ]
    ///    },
    ///    "gt_score_details": {
    ///      "type": "object"
    ///    },
    ///    "holders": {
    ///      "type": "object"
    ///    },
    ///    "is_honeypot": {
    ///      "oneOf": [
    ///        {
    ///          "type": "boolean"
    ///        },
    ///        {
    ///          "type": "string"
    ///        }
    ///      ]
    ///    },
    ///    "mint_authority": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    },
    ///    "symbol": {
    ///      "type": "string"
    ///    },
    ///    "telegram_handle": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "twitter_handle": {
    ///      "type": [
    ///        "string",
    ///        "null"
    ///      ]
    ///    },
    ///    "websites": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct TokenInfoDataAttributes {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub categories: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub coingecko_coin_id: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub description: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub developer_address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub developer_holding_percentage: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub discord_url: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub freeze_authority: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub gt_score: ::std::option::Option<f64>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub gt_score_details: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::serde_json::Map::is_empty")]
        pub holders: ::serde_json::Map<::std::string::String, ::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub is_honeypot: ::std::option::Option<TokenInfoDataAttributesIsHoneypot>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub mint_authority: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub symbol: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub telegram_handle: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub twitter_handle: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub websites: ::std::vec::Vec<::std::string::String>,
    }
    impl ::std::default::Default for TokenInfoDataAttributes {
        fn default() -> Self {
            Self {
                address: Default::default(),
                categories: Default::default(),
                coingecko_coin_id: Default::default(),
                description: Default::default(),
                developer_address: Default::default(),
                developer_holding_percentage: Default::default(),
                discord_url: Default::default(),
                freeze_authority: Default::default(),
                gt_score: Default::default(),
                gt_score_details: Default::default(),
                holders: Default::default(),
                is_honeypot: Default::default(),
                mint_authority: Default::default(),
                name: Default::default(),
                symbol: Default::default(),
                telegram_handle: Default::default(),
                twitter_handle: Default::default(),
                websites: Default::default(),
            }
        }
    }
    ///`TokenInfoDataAttributesIsHoneypot`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "oneOf": [
    ///    {
    ///      "type": "boolean"
    ///    },
    ///    {
    ///      "type": "string"
    ///    }
    ///  ]
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    #[serde(untagged)]
    pub enum TokenInfoDataAttributesIsHoneypot {
        Boolean(bool),
        String(::std::string::String),
    }
    impl ::std::fmt::Display for TokenInfoDataAttributesIsHoneypot {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match self {
                Self::Boolean(x) => x.fmt(f),
                Self::String(x) => x.fmt(f),
            }
        }
    }
    impl ::std::convert::From<bool> for TokenInfoDataAttributesIsHoneypot {
        fn from(value: bool) -> Self {
            Self::Boolean(value)
        }
    }
    ///`TokenInfoRecentlyUpdated`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": [
    ///        {
    ///          "attributes": {
    ///            "address": "8kgMCX7ezivU472eXaiGRu8xzNetpdxKDvNQBwKRipxi",
    ///            "banner_image_url": null,
    ///            "coingecko_coin_id": null,
    ///            "decimals": 6,
    ///            "description": "Our mission is to help the IRL Wizards of Doge go viral in support of Elon’s DOGE initiative and to propagate wizards throughout the realm",
    ///            "discord_url": null,
    ///            "gt_score": 62.50770642201834,
    ///            "image_url": "https://assets.geckoterminal.com/23fwhe56einulc0vqgs6bhkfusfr",
    ///            "metadata_updated_at": "2025-05-26T07:47:20Z",
    ///            "name": "WIZARDS OF DOGE",
    ///            "symbol": "WOD",
    ///            "telegram_handle": null,
    ///            "twitter_handle": "wizzoordofwod",
    ///            "websites": [
    ///              "https://wizzoordofwod.com"
    ///            ]
    ///          },
    ///          "id": "solana_8kgMCX7ezivU472eXaiGRu8xzNetpdxKDvNQBwKRipxi",
    ///          "relationships": {
    ///            "network": {
    ///              "data": {
    ///                "id": "solana",
    ///                "type": "network"
    ///              }
    ///            }
    ///          },
    ///          "type": "token"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "data": {
    ///      "type": "array",
    ///      "properties": {
    ///        "attributes": {
    ///          "type": "object",
    ///          "required": [
    ///            "address",
    ///            "coingecko_coin_id",
    ///            "description",
    ///            "discord_url",
    ///            "gt_score",
    ///            "image_url",
    ///            "name",
    ///            "symbol",
    ///            "telegram_handle",
    ///            "twitter_handle",
    ///            "websites"
    ///          ],
    ///          "properties": {
    ///            "address": {
    ///              "type": "string"
    ///            },
    ///            "banner_image_url": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "categories": {
    ///              "type": [
    ///                "array",
    ///                "null"
    ///              ],
    ///              "items": {
    ///                "type": "string"
    ///              }
    ///            },
    ///            "coingecko_coin_id": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "decimals": {
    ///              "type": "integer"
    ///            },
    ///            "description": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "discord_url": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "gt_category_ids": {
    ///              "type": [
    ///                "array",
    ///                "null"
    ///              ],
    ///              "items": {
    ///                "type": "string"
    ///              }
    ///            },
    ///            "gt_score": {
    ///              "type": [
    ///                "number",
    ///                "null"
    ///              ],
    ///              "format": "float"
    ///            },
    ///            "image_url": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "metadata_updated_at": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ],
    ///              "format": "datetime"
    ///            },
    ///            "name": {
    ///              "type": "string"
    ///            },
    ///            "symbol": {
    ///              "type": "string"
    ///            },
    ///            "telegram_handle": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "twitter_handle": {
    ///              "type": [
    ///                "string",
    ///                "null"
    ///              ]
    ///            },
    ///            "websites": {
    ///              "type": "array",
    ///              "items": {
    ///                "type": "string"
    ///              }
    ///            }
    ///          }
    ///        },
    ///        "id": {
    ///          "type": "string"
    ///        },
    ///        "type": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "relationships": {
    ///      "$ref": "#/components/schemas/relationships"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct TokenInfoRecentlyUpdated {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<::serde_json::Value>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub relationships: ::std::option::Option<Relationships>,
    }
    impl ::std::default::Default for TokenInfoRecentlyUpdated {
        fn default() -> Self {
            Self {
                data: Default::default(),
                relationships: Default::default(),
            }
        }
    }
    ///`TokenRelationships`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "top_pools": {
    ///      "type": "object",
    ///      "properties": {
    ///        "data": {
    ///          "type": "array",
    ///          "items": {
    ///            "type": "object",
    ///            "properties": {
    ///              "id": {
    ///                "type": "string"
    ///              },
    ///              "type": {
    ///                "type": "string"
    ///              }
    ///            }
    ///          }
    ///        }
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct TokenRelationships {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub top_pools: ::std::option::Option<TokenRelationshipsTopPools>,
    }
    impl ::std::default::Default for TokenRelationships {
        fn default() -> Self {
            Self {
                top_pools: Default::default(),
            }
        }
    }
    ///`TokenRelationshipsTopPools`
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
    ///        "type": "object",
    ///        "properties": {
    ///          "id": {
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
    pub struct TokenRelationshipsTopPools {
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub data: ::std::vec::Vec<TokenRelationshipsTopPoolsDataItem>,
    }
    impl ::std::default::Default for TokenRelationshipsTopPools {
        fn default() -> Self {
            Self {
                data: Default::default(),
            }
        }
    }
    ///`TokenRelationshipsTopPoolsDataItem`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
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
    pub struct TokenRelationshipsTopPoolsDataItem {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for TokenRelationshipsTopPoolsDataItem {
        fn default() -> Self {
            Self {
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`TokenType`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "string",
    ///  "enum": [
    ///    "token"
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
    pub enum TokenType {
        #[serde(rename = "token")]
        Token,
    }
    impl ::std::fmt::Display for TokenType {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Token => f.write_str("token"),
            }
        }
    }
    impl ::std::str::FromStr for TokenType {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "token" => Ok(Self::Token),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for TokenType {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for TokenType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for TokenType {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`Trade`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "examples": [
    ///    {
    ///      "data": [
    ///        {
    ///          "attributes": {
    ///            "block_number": 19612255,
    ///            "block_timestamp": "2024-04-08T16:52:35Z",
    ///            "from_token_address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
    ///            "from_token_amount": "1.51717616246451",
    ///            "kind": "buy",
    ///            "price_from_in_currency_token": "1.0",
    ///            "price_from_in_usd": "3656.8970003075",
    ///            "price_to_in_currency_token": "0.000274100995437363",
    ///            "price_to_in_usd": "1.00235910799619",
    ///            "to_token_address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    ///            "to_token_amount": "5535.099061",
    ///            "tx_from_address": "0x42c037c594eefeca741e9dd66af91e7ffd930872",
    ///            "tx_hash": "0x0b8ac5a16c291832c1b4d5f0d8ef2d9d58e207cd8132c32392295617daa4d422",
    ///            "volume_in_usd": "5548.15695745452"
    ///          },
    ///          "id": "eth_19612255_0x0b8ac5a16c291832c1b4d5f0d8ef2d9d58e207cd8132c32392295617daa4d422_158_1712595165",
    ///          "type": "trade"
    ///        }
    ///      ]
    ///    }
    ///  ],
    ///  "type": "object",
    ///  "properties": {
    ///    "attributes": {
    ///      "properties": {
    ///        "block_number": {
    ///          "type": "integer"
    ///        },
    ///        "block_timestamp": {
    ///          "type": "string",
    ///          "format": "datetime"
    ///        },
    ///        "from_token_address": {
    ///          "type": "string"
    ///        },
    ///        "from_token_amount": {
    ///          "type": "string"
    ///        },
    ///        "kind": {
    ///          "type": "string"
    ///        },
    ///        "price_from_in_currency_token": {
    ///          "type": "string"
    ///        },
    ///        "price_from_in_usd": {
    ///          "type": "string"
    ///        },
    ///        "price_to_in_currency_token": {
    ///          "type": "string"
    ///        },
    ///        "price_to_in_usd": {
    ///          "type": "string"
    ///        },
    ///        "to_token_address": {
    ///          "type": "string"
    ///        },
    ///        "to_token_amount": {
    ///          "type": "string"
    ///        },
    ///        "tx_from_address": {
    ///          "type": "string"
    ///        },
    ///        "tx_hash": {
    ///          "type": "string"
    ///        },
    ///        "volume_in_usd": {
    ///          "type": "string"
    ///        }
    ///      }
    ///    },
    ///    "id": {
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
    pub struct Trade {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub attributes: ::std::option::Option<TradeAttributes>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for Trade {
        fn default() -> Self {
            Self {
                attributes: Default::default(),
                id: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`TradeAttributes`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "properties": {
    ///    "block_number": {
    ///      "type": "integer"
    ///    },
    ///    "block_timestamp": {
    ///      "type": "string",
    ///      "format": "datetime"
    ///    },
    ///    "from_token_address": {
    ///      "type": "string"
    ///    },
    ///    "from_token_amount": {
    ///      "type": "string"
    ///    },
    ///    "kind": {
    ///      "type": "string"
    ///    },
    ///    "price_from_in_currency_token": {
    ///      "type": "string"
    ///    },
    ///    "price_from_in_usd": {
    ///      "type": "string"
    ///    },
    ///    "price_to_in_currency_token": {
    ///      "type": "string"
    ///    },
    ///    "price_to_in_usd": {
    ///      "type": "string"
    ///    },
    ///    "to_token_address": {
    ///      "type": "string"
    ///    },
    ///    "to_token_amount": {
    ///      "type": "string"
    ///    },
    ///    "tx_from_address": {
    ///      "type": "string"
    ///    },
    ///    "tx_hash": {
    ///      "type": "string"
    ///    },
    ///    "volume_in_usd": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct TradeAttributes {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub block_number: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub block_timestamp: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub from_token_address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub from_token_amount: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub kind: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_from_in_currency_token: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_from_in_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_to_in_currency_token: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub price_to_in_usd: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub to_token_address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub to_token_amount: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tx_from_address: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub tx_hash: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub volume_in_usd: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for TradeAttributes {
        fn default() -> Self {
            Self {
                block_number: Default::default(),
                block_timestamp: Default::default(),
                from_token_address: Default::default(),
                from_token_amount: Default::default(),
                kind: Default::default(),
                price_from_in_currency_token: Default::default(),
                price_from_in_usd: Default::default(),
                price_to_in_currency_token: Default::default(),
                price_to_in_usd: Default::default(),
                to_token_address: Default::default(),
                to_token_amount: Default::default(),
                tx_from_address: Default::default(),
                tx_hash: Default::default(),
                volume_in_usd: Default::default(),
            }
        }
    }
}
#[derive(Clone, Debug)]
/**Client for GeckoTerminal API V2

GeckoTerminal Public API endpoints.

## Beta Release
The API is in its Beta release, and is subject to frequent changes.
However, we aim to provide minimal disruption, and setting the request version would help avoid unexpected issues.

**Please subscribe via [this form](https://forms.gle/jSMu4jLQBXeiVD1U9) to be notified of important API updates.**

## Base URL
All endpoints below use the base URL: `https://api.geckoterminal.com/api/v2`

## Versioning
It is recommended to set the API version via the `Accept` header.
The current version is **20230203**.

For example, to specify the current version, set header `Accept: application/json;version=20230203`.

_If no version is specified, the latest version will be used._

## Data Freshness
All endpoints listed below are cached for **1 minute**

All data is updated **as fast as 2-3 seconds** after a transaction is confirmed on the blockchain, subject to the network's availability.

## Rate Limit
This Public API is limited to approximately 10 calls/minute, which may fluctuate based on network traffic.
For higher and more stable rate limits please subscribe to any CoinGecko API paid plan to access higher rate limit for GeckoTerminal endpoints (known as /onchain endpoints) or learn more at [CoinGecko](https://www.coingecko.com/en/api/pricing).

To share with us your feedback about the public API, let us know [here](https://forms.gle/jSMu4jLQBXeiVD1U9)!


Version: v2-beta*/
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
        "v2-beta"
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
    /**Pool OHLCV chart by Pool Address

    This endpoint allows you to **get the OHLCV chart (Open, High, Low, Close, Volume) of a pool based on the provided pool address on a network**

    Sends a `GET` request to `/networks/{network}/pools/{pool_address}/ohlcv/{timeframe}`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `pool_address`: pool contract address
    - `timeframe`: timeframe of the OHLCV chart
    - `aggregate`: time period to aggregate for each OHLCV<br><br><b>Available values (day):</b> `1`<br><br><b>Available values (hour):</b> `1`, `4`, `12`<br><br><b>Available values (minute):</b> `1`, `5`, `15`
    - `before_timestamp`: return OHLCV data before this timestamp (integer seconds since epoch)
    - `currency`: return OHLCV in USD or quote token
    - `include_empty_intervals`: include empty intervals with no trade data
    - `limit`: number of OHLCV results to return, maximum 1000
    - `token`: return OHLCV for token<br>use this to invert the chart<br>Available values: 'base', 'quote', or token address
    */
    pub async fn getnetworks_network_pools_pooladdress_ohlcv_timeframe<'a>(
        &'a self,
        network: &'a str,
        pool_address: &'a str,
        timeframe: types::GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe,
        aggregate: Option<&'a str>,
        before_timestamp: Option<&'a str>,
        currency: Option<types::GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency>,
        include_empty_intervals: Option<bool>,
        limit: Option<i64>,
        token: Option<&'a str>,
    ) -> Result<ResponseValue<types::Ohlcv>, Error<()>> {
        let url = format!(
            "{}/networks/{}/pools/{}/ohlcv/{}",
            self.baseurl,
            encode_path(&network.to_string()),
            encode_path(&pool_address.to_string()),
            encode_path(&timeframe.to_string()),
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
            .query(&progenitor_client::QueryParam::new("aggregate", &aggregate))
            .query(&progenitor_client::QueryParam::new(
                "before_timestamp",
                &before_timestamp,
            ))
            .query(&progenitor_client::QueryParam::new("currency", &currency))
            .query(&progenitor_client::QueryParam::new(
                "include_empty_intervals",
                &include_empty_intervals,
            ))
            .query(&progenitor_client::QueryParam::new("limit", &limit))
            .query(&progenitor_client::QueryParam::new("token", &token))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_pools_pooladdress_ohlcv_timeframe",
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
    /**Supported Dexes List by Network (ID Map)

    This endpoint allows you to **query all the supported decentralized exchanges (DEXs) based on the provided network on GeckoTerminal**

    Sends a `GET` request to `/networks/{network}/dexes`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `page`: page through results
    */
    pub async fn getnetworks_network_dexes<'a>(
        &'a self,
        network: &'a str,
        page: Option<i64>,
    ) -> Result<ResponseValue<types::DexesList>, Error<()>> {
        let url = format!(
            "{}/networks/{}/dexes",
            self.baseurl,
            encode_path(&network.to_string()),
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
            .query(&progenitor_client::QueryParam::new("page", &page))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_dexes",
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
    /**Supported Networks List (ID Map)

    This endpoint allows you to **query all the supported networks on GeckoTerminal**

    Sends a `GET` request to `/networks`

    Arguments:
    - `page`: page through results (maximum: 10)
    */
    pub async fn getnetworks<'a>(
        &'a self,
        page: Option<i64>,
    ) -> Result<ResponseValue<types::NetworksList>, Error<()>> {
        let url = format!("{}/networks", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("page", &page))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks",
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
    /**Trending Pools List

    This endpoint allows you to **query all the trending pools across all networks on GeckoTerminal**

    Sends a `GET` request to `/networks/trending_pools`

    Arguments:
    - `duration`: duration to sort trending list by
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`, `network`
    - `include_gt_community_data`: include GT community data
    - `page`: page through results (maximum: 10)
    */
    pub async fn getnetworks_trendingpools<'a>(
        &'a self,
        duration: Option<types::GetnetworksTrendingpoolsDuration>,
        include: Option<&'a str>,
        include_gt_community_data: Option<bool>,
        page: Option<i64>,
    ) -> Result<ResponseValue<types::Pool>, Error<()>> {
        let url = format!("{}/networks/trending_pools", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("duration", &duration))
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_gt_community_data",
                &include_gt_community_data,
            ))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_trendingpools",
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
    /**Trending Pools by Network

    This endpoint allows you to **query the trending pools based on the provided network**

    Sends a `GET` request to `/networks/{network}/trending_pools`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `duration`: duration to sort trending list by
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`
    - `include_gt_community_data`: include GT community data
    - `page`: page through results (maximum: 10)
    */
    pub async fn getnetworks_network_trendingpools<'a>(
        &'a self,
        network: &'a str,
        duration: Option<types::GetnetworksNetworkTrendingpoolsDuration>,
        include: Option<&'a str>,
        include_gt_community_data: Option<bool>,
        page: Option<i64>,
    ) -> Result<ResponseValue<types::Pool>, Error<()>> {
        let url = format!(
            "{}/networks/{}/trending_pools",
            self.baseurl,
            encode_path(&network.to_string()),
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
            .query(&progenitor_client::QueryParam::new("duration", &duration))
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_gt_community_data",
                &include_gt_community_data,
            ))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_trendingpools",
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
    /**Specific Pool Data by Pool Address

    This endpoint allows you to **query the specific pool based on the provided network and pool address**

    Sends a `GET` request to `/networks/{network}/pools/{address}`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `address`: pool address
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`
    - `include_composition`: include pool composition
    - `include_volume_breakdown`: include volume breakdown
    */
    pub async fn getnetworks_network_pools_address<'a>(
        &'a self,
        network: &'a str,
        address: &'a str,
        include: Option<&'a str>,
        include_composition: Option<bool>,
        include_volume_breakdown: Option<bool>,
    ) -> Result<ResponseValue<types::SinglePoolDetail>, Error<()>> {
        let url = format!(
            "{}/networks/{}/pools/{}",
            self.baseurl,
            encode_path(&network.to_string()),
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_composition",
                &include_composition,
            ))
            .query(&progenitor_client::QueryParam::new(
                "include_volume_breakdown",
                &include_volume_breakdown,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_pools_address",
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
    /**Multiple Pools Data by Pool Addresses

    This endpoint allows you to **query multiple pools based on the provided network and pool address**

    Sends a `GET` request to `/networks/{network}/pools/multi/{addresses}`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `addresses`: pool contract address, comma-separated if more than one pool contract address (up to 30 addresses)
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`
    - `include_composition`: include pool composition
    - `include_volume_breakdown`: include volume breakdown
    */
    pub async fn getnetworks_network_pools_multi_addresses<'a>(
        &'a self,
        network: &'a str,
        addresses: &'a str,
        include: Option<&'a str>,
        include_composition: Option<bool>,
        include_volume_breakdown: Option<bool>,
    ) -> Result<ResponseValue<types::MultiPoolDetails>, Error<()>> {
        let url = format!(
            "{}/networks/{}/pools/multi/{}",
            self.baseurl,
            encode_path(&network.to_string()),
            encode_path(&addresses.to_string()),
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_composition",
                &include_composition,
            ))
            .query(&progenitor_client::QueryParam::new(
                "include_volume_breakdown",
                &include_volume_breakdown,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_pools_multi_addresses",
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
    /**Top Pools by Network

    This endpoint allows you to **query all the top pools based on the provided network**

    Sends a `GET` request to `/networks/{network}/pools`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`
    - `include_gt_community_data`: include GT community data
    - `page`: page through results (maximum: 10)
    - `sort`: sort the pools by field
    */
    pub async fn getnetworks_network_pools<'a>(
        &'a self,
        network: &'a str,
        include: Option<&'a str>,
        include_gt_community_data: Option<bool>,
        page: Option<i64>,
        sort: Option<types::GetnetworksNetworkPoolsSort>,
    ) -> Result<ResponseValue<types::Pool>, Error<()>> {
        let url = format!(
            "{}/networks/{}/pools",
            self.baseurl,
            encode_path(&network.to_string()),
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_gt_community_data",
                &include_gt_community_data,
            ))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .query(&progenitor_client::QueryParam::new("sort", &sort))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_pools",
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
    /**Top Pools by Dex

    This endpoint allows you to **query all the top pools based on the provided network and decentralized exchange (DEX)**

    Sends a `GET` request to `/networks/{network}/dexes/{dex}/pools`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `dex`: DEX ID<br>*refers to [/networks/{network}/dexes](/api/v2/networks/eth/dexes)
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`
    - `include_gt_community_data`: include GT community data
    - `page`: page through results (maximum: 10)
    - `sort`: sort the pools by field
    */
    pub async fn getnetworks_network_dexes_dex_pools<'a>(
        &'a self,
        network: &'a str,
        dex: &'a str,
        include: Option<&'a str>,
        include_gt_community_data: Option<bool>,
        page: Option<i64>,
        sort: Option<types::GetnetworksNetworkDexesDexPoolsSort>,
    ) -> Result<ResponseValue<types::Pool>, Error<()>> {
        let url = format!(
            "{}/networks/{}/dexes/{}/pools",
            self.baseurl,
            encode_path(&network.to_string()),
            encode_path(&dex.to_string()),
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_gt_community_data",
                &include_gt_community_data,
            ))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .query(&progenitor_client::QueryParam::new("sort", &sort))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_dexes_dex_pools",
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
    /**Top Pools by Token Address

    This endpoint allows you to **query top pools based on the provided token contract address on a network**

    Sends a `GET` request to `/networks/{network}/tokens/{token_address}/pools`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `token_address`: token contract address
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`
    - `include_inactive_source`: include inactive pools when sourcing for token's top pool
    - `page`: page through results (maximum: 10)
    - `sort`: sort the pools by field
    */
    pub async fn getnetworks_network_tokens_tokenaddress_pools<'a>(
        &'a self,
        network: &'a str,
        token_address: &'a str,
        include: Option<&'a str>,
        include_inactive_source: Option<bool>,
        page: Option<i64>,
        sort: Option<types::GetnetworksNetworkTokensTokenaddressPoolsSort>,
    ) -> Result<ResponseValue<types::Pool>, Error<()>> {
        let url = format!(
            "{}/networks/{}/tokens/{}/pools",
            self.baseurl,
            encode_path(&network.to_string()),
            encode_path(&token_address.to_string()),
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_inactive_source",
                &include_inactive_source,
            ))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .query(&progenitor_client::QueryParam::new("sort", &sort))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_tokens_tokenaddress_pools",
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
    /**New Pools by Network

    This endpoint allows you to **query all the latest pools based on provided network**

    Sends a `GET` request to `/networks/{network}/new_pools`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`
    - `include_gt_community_data`: include GT community data
    - `page`: page through results (maximum: 10)
    */
    pub async fn getnetworks_network_newpools<'a>(
        &'a self,
        network: &'a str,
        include: Option<&'a str>,
        include_gt_community_data: Option<bool>,
        page: Option<i64>,
    ) -> Result<ResponseValue<types::Pool>, Error<()>> {
        let url = format!(
            "{}/networks/{}/new_pools",
            self.baseurl,
            encode_path(&network.to_string()),
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_gt_community_data",
                &include_gt_community_data,
            ))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_newpools",
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
    /**New Pools List

    This endpoint allows you to **query all the latest pools across all networks on GeckoTerminal**

    Sends a `GET` request to `/networks/new_pools`

    Arguments:
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`, `network`
    - `include_gt_community_data`: include GT community data
    - `page`: page through results (maximum: 10)
    */
    pub async fn getnetworks_newpools<'a>(
        &'a self,
        include: Option<&'a str>,
        include_gt_community_data: Option<bool>,
        page: Option<i64>,
    ) -> Result<ResponseValue<types::Pool>, Error<()>> {
        let url = format!("{}/networks/new_pools", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_gt_community_data",
                &include_gt_community_data,
            ))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_newpools",
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
    /**Search Pools

    This endpoint allows you to **search for pools on a network**

    Sends a `GET` request to `/search/pools`

    Arguments:
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `base_token`, `quote_token`, `dex`
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `page`: page through results (maximum: 10)
    - `query`: search query
    */
    pub async fn getsearch_pools<'a>(
        &'a self,
        include: Option<&'a str>,
        network: Option<&'a str>,
        page: Option<i64>,
        query: Option<&'a str>,
    ) -> Result<ResponseValue<types::Pool>, Error<()>> {
        let url = format!("{}/search/pools", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new("network", &network))
            .query(&progenitor_client::QueryParam::new("page", &page))
            .query(&progenitor_client::QueryParam::new("query", &query))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getsearch_pools",
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
    /**Past 24 Hour Trades by Pool Address

    This endpoint allows you to **query the last 300 trades in the past 24 hours based on the provided pool address**

    Sends a `GET` request to `/networks/{network}/pools/{pool_address}/trades`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `pool_address`: pool contract address
    - `token`: return trades for token<br>use this to invert the data<br>Available values: 'base', 'quote', or token address
    - `trade_volume_in_usd_greater_than`: filter trades by trade volume in USD greater than this value
    */
    pub async fn getnetworks_network_pools_pooladdress_trades<'a>(
        &'a self,
        network: &'a str,
        pool_address: &'a str,
        token: Option<&'a str>,
        trade_volume_in_usd_greater_than: Option<f64>,
    ) -> Result<ResponseValue<types::Trade>, Error<()>> {
        let url = format!(
            "{}/networks/{}/pools/{}/trades",
            self.baseurl,
            encode_path(&network.to_string()),
            encode_path(&pool_address.to_string()),
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
            .query(&progenitor_client::QueryParam::new("token", &token))
            .query(&progenitor_client::QueryParam::new(
                "trade_volume_in_usd_greater_than",
                &trade_volume_in_usd_greater_than,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_pools_pooladdress_trades",
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
    /**Token Data by Token Address

    This endpoint allows you to **query specific token data based on the provided token contract address on a network**

    Sends a `GET` request to `/networks/{network}/tokens/{address}`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `address`: token contract address
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `top_pools`
    - `include_composition`: include pool composition
    - `include_inactive_source`: include inactive pools when sourcing for token's top pool
    */
    pub async fn getnetworks_network_tokens_address<'a>(
        &'a self,
        network: &'a str,
        address: &'a str,
        include: Option<&'a str>,
        include_composition: Option<bool>,
        include_inactive_source: Option<bool>,
    ) -> Result<ResponseValue<types::TokenDetail>, Error<()>> {
        let url = format!(
            "{}/networks/{}/tokens/{}",
            self.baseurl,
            encode_path(&network.to_string()),
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_composition",
                &include_composition,
            ))
            .query(&progenitor_client::QueryParam::new(
                "include_inactive_source",
                &include_inactive_source,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_tokens_address",
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
    /**Tokens Data by Token Addresses

    This endpoint allows you to **query multiple tokens data based on the provided token contract addresses on a network**

    Sends a `GET` request to `/networks/{network}/tokens/multi/{addresses}`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `addresses`: token contract address, comma-separated if more than one token contract address (up to 30 addresses)
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `top_pools`
    - `include_composition`: include pool composition
    - `include_inactive_source`: include inactive pools when sourcing for token's top pool
    */
    pub async fn getnetworks_network_tokens_multi_addresses<'a>(
        &'a self,
        network: &'a str,
        addresses: &'a str,
        include: Option<&'a str>,
        include_composition: Option<bool>,
        include_inactive_source: Option<bool>,
    ) -> Result<ResponseValue<types::MultiTokenDetail>, Error<()>> {
        let url = format!(
            "{}/networks/{}/tokens/multi/{}",
            self.baseurl,
            encode_path(&network.to_string()),
            encode_path(&addresses.to_string()),
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new(
                "include_composition",
                &include_composition,
            ))
            .query(&progenitor_client::QueryParam::new(
                "include_inactive_source",
                &include_inactive_source,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_tokens_multi_addresses",
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
    /**Token Info by Token Address

    This endpoint allows you to **query token metadata (name, symbol,  CoinGecko ID, image, socials, websites, description, etc.) based on a provided token contract address on a network**

    Sends a `GET` request to `/networks/{network}/tokens/{address}/info`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `address`: token contract address
    */
    pub async fn getnetworks_network_tokens_address_info<'a>(
        &'a self,
        network: &'a str,
        address: &'a str,
    ) -> Result<ResponseValue<types::TokenInfo>, Error<()>> {
        let url = format!(
            "{}/networks/{}/tokens/{}/info",
            self.baseurl,
            encode_path(&network.to_string()),
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
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_tokens_address_info",
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
    /**Pool Tokens Info by Pool Address

    This endpoint allows you to **query pool metadata (base and quote token details, image, socials, websites, description, contract address, etc.) based on a provided pool contract address on a network**

    Sends a `GET` request to `/networks/{network}/pools/{pool_address}/info`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `pool_address`: pool contract address
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `pool`
    */
    pub async fn getnetworks_network_pools_pooladdress_info<'a>(
        &'a self,
        network: &'a str,
        pool_address: &'a str,
        include: Option<&'a str>,
    ) -> Result<ResponseValue<types::PoolTokensInfo>, Error<()>> {
        let url = format!(
            "{}/networks/{}/pools/{}/info",
            self.baseurl,
            encode_path(&network.to_string()),
            encode_path(&pool_address.to_string()),
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getnetworks_network_pools_pooladdress_info",
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
    /**Most Recently Updated Tokens List

    This endpoint allows you to **query 100 most recently updated tokens info of a specific network or across all networks on GeckoTerminal**

    Sends a `GET` request to `/tokens/info_recently_updated`

    Arguments:
    - `include`: attributes to include, comma-separated if more than one to include<br><br><b>Available values:</b> `network`
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    */
    pub async fn gettokens_inforecentlyupdated<'a>(
        &'a self,
        include: Option<&'a str>,
        network: Option<&'a str>,
    ) -> Result<ResponseValue<types::TokenInfoRecentlyUpdated>, Error<()>> {
        let url = format!("{}/tokens/info_recently_updated", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("include", &include))
            .query(&progenitor_client::QueryParam::new("network", &network))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "gettokens_inforecentlyupdated",
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
    /**Token Price by Token Addresses

    This endpoint allows you to **get token price based on the provided token contract address on a network**

    Sends a `GET` request to `/simple/networks/{network}/token_price/{addresses}`

    Arguments:
    - `network`: network ID<br>*refers to [/networks](/api/v2/networks)
    - `addresses`: token contract address, comma-separated if more than one token contract address (up to 30 addresses)
    - `include_24hr_price_change`: include 24hr price change
    - `include_24hr_vol`: include 24hr volume
    - `include_inactive_source`: include inactive pools when sourcing for token's top pool
    - `include_market_cap`: include market capitalization
    - `include_total_reserve_in_usd`: include total reserve in USD
    - `mcap_fdv_fallback`: return FDV if market cap is not available
    */
    pub async fn getsimple_networks_network_tokenprice_addresses<'a>(
        &'a self,
        network: &'a str,
        addresses: &'a str,
        include_24hr_price_change: Option<bool>,
        include_24hr_vol: Option<bool>,
        include_inactive_source: Option<bool>,
        include_market_cap: Option<bool>,
        include_total_reserve_in_usd: Option<bool>,
        mcap_fdv_fallback: Option<bool>,
    ) -> Result<ResponseValue<types::GetsimpleNetworksNetworkTokenpriceAddressesResponse>, Error<()>>
    {
        let url = format!(
            "{}/simple/networks/{}/token_price/{}",
            self.baseurl,
            encode_path(&network.to_string()),
            encode_path(&addresses.to_string()),
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
            .query(&progenitor_client::QueryParam::new(
                "include_24hr_price_change",
                &include_24hr_price_change,
            ))
            .query(&progenitor_client::QueryParam::new(
                "include_24hr_vol",
                &include_24hr_vol,
            ))
            .query(&progenitor_client::QueryParam::new(
                "include_inactive_source",
                &include_inactive_source,
            ))
            .query(&progenitor_client::QueryParam::new(
                "include_market_cap",
                &include_market_cap,
            ))
            .query(&progenitor_client::QueryParam::new(
                "include_total_reserve_in_usd",
                &include_total_reserve_in_usd,
            ))
            .query(&progenitor_client::QueryParam::new(
                "mcap_fdv_fallback",
                &mcap_fdv_fallback,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "getsimple_networks_network_tokenprice_addresses",
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
