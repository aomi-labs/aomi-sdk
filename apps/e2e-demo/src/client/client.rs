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
    ///`ApiResponse`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "code": {
    ///      "type": "integer",
    ///      "format": "int32"
    ///    },
    ///    "message": {
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
    pub struct ApiResponse {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub code: ::std::option::Option<i32>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub message: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "type",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub type_: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for ApiResponse {
        fn default() -> Self {
            Self {
                code: Default::default(),
                message: Default::default(),
                type_: Default::default(),
            }
        }
    }
    ///`Category`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "name": {
    ///      "examples": [
    ///        "Dogs"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Category {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for Category {
        fn default() -> Self {
            Self {
                id: Default::default(),
                name: Default::default(),
            }
        }
    }
    ///`FindPetsByStatusStatus`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "default": "available",
    ///  "type": "string",
    ///  "enum": [
    ///    "available",
    ///    "pending",
    ///    "sold"
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
    pub enum FindPetsByStatusStatus {
        #[serde(rename = "available")]
        Available,
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "sold")]
        Sold,
    }
    impl ::std::fmt::Display for FindPetsByStatusStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Available => f.write_str("available"),
                Self::Pending => f.write_str("pending"),
                Self::Sold => f.write_str("sold"),
            }
        }
    }
    impl ::std::str::FromStr for FindPetsByStatusStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "available" => Ok(Self::Available),
                "pending" => Ok(Self::Pending),
                "sold" => Ok(Self::Sold),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for FindPetsByStatusStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for FindPetsByStatusStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for FindPetsByStatusStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::default::Default for FindPetsByStatusStatus {
        fn default() -> Self {
            FindPetsByStatusStatus::Available
        }
    }
    ///`Order`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "complete": {
    ///      "type": "boolean"
    ///    },
    ///    "id": {
    ///      "examples": [
    ///        10
    ///      ],
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "petId": {
    ///      "examples": [
    ///        198772
    ///      ],
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "quantity": {
    ///      "examples": [
    ///        7
    ///      ],
    ///      "type": "integer",
    ///      "format": "int32"
    ///    },
    ///    "shipDate": {
    ///      "type": "string",
    ///      "format": "date-time"
    ///    },
    ///    "status": {
    ///      "description": "Order Status",
    ///      "examples": [
    ///        "approved"
    ///      ],
    ///      "type": "string",
    ///      "enum": [
    ///        "placed",
    ///        "approved",
    ///        "delivered"
    ///      ]
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Order {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub complete: ::std::option::Option<bool>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<i64>,
        #[serde(
            rename = "petId",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub pet_id: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub quantity: ::std::option::Option<i32>,
        #[serde(
            rename = "shipDate",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub ship_date: ::std::option::Option<::chrono::DateTime<::chrono::offset::Utc>>,
        ///Order Status
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<OrderStatus>,
    }
    impl ::std::default::Default for Order {
        fn default() -> Self {
            Self {
                complete: Default::default(),
                id: Default::default(),
                pet_id: Default::default(),
                quantity: Default::default(),
                ship_date: Default::default(),
                status: Default::default(),
            }
        }
    }
    ///Order Status
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "Order Status",
    ///  "examples": [
    ///    "approved"
    ///  ],
    ///  "type": "string",
    ///  "enum": [
    ///    "placed",
    ///    "approved",
    ///    "delivered"
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
    pub enum OrderStatus {
        #[serde(rename = "placed")]
        Placed,
        #[serde(rename = "approved")]
        Approved,
        #[serde(rename = "delivered")]
        Delivered,
    }
    impl ::std::fmt::Display for OrderStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Placed => f.write_str("placed"),
                Self::Approved => f.write_str("approved"),
                Self::Delivered => f.write_str("delivered"),
            }
        }
    }
    impl ::std::str::FromStr for OrderStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "placed" => Ok(Self::Placed),
                "approved" => Ok(Self::Approved),
                "delivered" => Ok(Self::Delivered),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for OrderStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for OrderStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for OrderStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`Pet`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "required": [
    ///    "name",
    ///    "photoUrls"
    ///  ],
    ///  "properties": {
    ///    "category": {
    ///      "$ref": "#/components/schemas/Category"
    ///    },
    ///    "id": {
    ///      "examples": [
    ///        10
    ///      ],
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "name": {
    ///      "examples": [
    ///        "doggie"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "photoUrls": {
    ///      "type": "array",
    ///      "items": {
    ///        "type": "string"
    ///      }
    ///    },
    ///    "status": {
    ///      "description": "pet status in the store",
    ///      "type": "string",
    ///      "enum": [
    ///        "available",
    ///        "pending",
    ///        "sold"
    ///      ]
    ///    },
    ///    "tags": {
    ///      "type": "array",
    ///      "items": {
    ///        "$ref": "#/components/schemas/Tag"
    ///      }
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Pet {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub category: ::std::option::Option<Category>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<i64>,
        pub name: ::std::string::String,
        #[serde(rename = "photoUrls")]
        pub photo_urls: ::std::vec::Vec<::std::string::String>,
        ///pet status in the store
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub status: ::std::option::Option<PetStatus>,
        #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
        pub tags: ::std::vec::Vec<Tag>,
    }
    ///pet status in the store
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "description": "pet status in the store",
    ///  "type": "string",
    ///  "enum": [
    ///    "available",
    ///    "pending",
    ///    "sold"
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
    pub enum PetStatus {
        #[serde(rename = "available")]
        Available,
        #[serde(rename = "pending")]
        Pending,
        #[serde(rename = "sold")]
        Sold,
    }
    impl ::std::fmt::Display for PetStatus {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
            match *self {
                Self::Available => f.write_str("available"),
                Self::Pending => f.write_str("pending"),
                Self::Sold => f.write_str("sold"),
            }
        }
    }
    impl ::std::str::FromStr for PetStatus {
        type Err = self::error::ConversionError;
        fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            match value {
                "available" => Ok(Self::Available),
                "pending" => Ok(Self::Pending),
                "sold" => Ok(Self::Sold),
                _ => Err("invalid value".into()),
            }
        }
    }
    impl ::std::convert::TryFrom<&str> for PetStatus {
        type Error = self::error::ConversionError;
        fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<&::std::string::String> for PetStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: &::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    impl ::std::convert::TryFrom<::std::string::String> for PetStatus {
        type Error = self::error::ConversionError;
        fn try_from(
            value: ::std::string::String,
        ) -> ::std::result::Result<Self, self::error::ConversionError> {
            value.parse()
        }
    }
    ///`Tag`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "id": {
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "name": {
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct Tag {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<i64>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub name: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for Tag {
        fn default() -> Self {
            Self {
                id: Default::default(),
                name: Default::default(),
            }
        }
    }
    ///`User`
    ///
    /// <details><summary>JSON schema</summary>
    ///
    /// ```json
    ///{
    ///  "type": "object",
    ///  "properties": {
    ///    "email": {
    ///      "examples": [
    ///        "john@email.com"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "firstName": {
    ///      "examples": [
    ///        "John"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "id": {
    ///      "examples": [
    ///        10
    ///      ],
    ///      "type": "integer",
    ///      "format": "int64"
    ///    },
    ///    "lastName": {
    ///      "examples": [
    ///        "James"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "password": {
    ///      "examples": [
    ///        "12345"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "phone": {
    ///      "examples": [
    ///        "12345"
    ///      ],
    ///      "type": "string"
    ///    },
    ///    "userStatus": {
    ///      "description": "User Status",
    ///      "examples": [
    ///        1
    ///      ],
    ///      "type": "integer",
    ///      "format": "int32"
    ///    },
    ///    "username": {
    ///      "examples": [
    ///        "theUser"
    ///      ],
    ///      "type": "string"
    ///    }
    ///  }
    ///}
    /// ```
    /// </details>
    #[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
    pub struct User {
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub email: ::std::option::Option<::std::string::String>,
        #[serde(
            rename = "firstName",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub first_name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub id: ::std::option::Option<i64>,
        #[serde(
            rename = "lastName",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub last_name: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub password: ::std::option::Option<::std::string::String>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub phone: ::std::option::Option<::std::string::String>,
        ///User Status
        #[serde(
            rename = "userStatus",
            default,
            skip_serializing_if = "::std::option::Option::is_none"
        )]
        pub user_status: ::std::option::Option<i32>,
        #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
        pub username: ::std::option::Option<::std::string::String>,
    }
    impl ::std::default::Default for User {
        fn default() -> Self {
            Self {
                email: Default::default(),
                first_name: Default::default(),
                id: Default::default(),
                last_name: Default::default(),
                password: Default::default(),
                phone: Default::default(),
                user_status: Default::default(),
                username: Default::default(),
            }
        }
    }
}
#[derive(Clone, Debug)]
/**Client for Swagger Petstore - OpenAPI 3.0

This is a sample Pet Store Server based on the OpenAPI 3.0 specification.  You can find out more about
Swagger at [https://swagger.io](https://swagger.io). In the third iteration of the pet store, we've switched to the design first approach!
You can now help us improve the API whether it's by making changes to the definition itself or to the code.
That way, with time, we can improve the API in general, and expose some of the new features in OAS3.

Some useful links:
- [The Pet Store repository](https://github.com/swagger-api/swagger-petstore)
- [The source API definition for the Pet Store](https://github.com/swagger-api/swagger-petstore/blob/master/src/main/resources/openapi.yaml)

https://swagger.io/terms/

Version: 1.0.27*/
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
        "1.0.27"
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
    /**Update an existing pet

    Update an existing pet by Id.

    Sends a `PUT` request to `/pet`

    Arguments:
    - `body`: Update an existent pet in the store
    */
    pub async fn update_pet<'a>(
        &'a self,
        body: &'a types::Pet,
    ) -> Result<ResponseValue<types::Pet>, Error<()>> {
        let url = format!("{}/pet", self.baseurl,);
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
            operation_id: "update_pet",
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
    /**Add a new pet to the store

    Add a new pet to the store.

    Sends a `POST` request to `/pet`

    Arguments:
    - `body`: Create a new pet in the store
    */
    pub async fn add_pet<'a>(
        &'a self,
        body: &'a types::Pet,
    ) -> Result<ResponseValue<types::Pet>, Error<()>> {
        let url = format!("{}/pet", self.baseurl,);
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
            operation_id: "add_pet",
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
    /**Finds Pets by status

    Multiple status values can be provided with comma separated strings.

    Sends a `GET` request to `/pet/findByStatus`

    Arguments:
    - `status`: Status values that need to be considered for filter
    */
    pub async fn find_pets_by_status<'a>(
        &'a self,
        status: types::FindPetsByStatusStatus,
    ) -> Result<ResponseValue<::std::vec::Vec<types::Pet>>, Error<()>> {
        let url = format!("{}/pet/findByStatus", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("status", &status))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "find_pets_by_status",
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
    /**Finds Pets by tags

    Multiple tags can be provided with comma separated strings. Use tag1, tag2, tag3 for testing.

    Sends a `GET` request to `/pet/findByTags`

    Arguments:
    - `tags`: Tags to filter by
    */
    pub async fn find_pets_by_tags<'a>(
        &'a self,
        tags: &'a ::std::vec::Vec<::std::string::String>,
    ) -> Result<ResponseValue<::std::vec::Vec<types::Pet>>, Error<()>> {
        let url = format!("{}/pet/findByTags", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("tags", &tags))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "find_pets_by_tags",
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
    /**Find pet by ID

    Returns a single pet.

    Sends a `GET` request to `/pet/{petId}`

    Arguments:
    - `pet_id`: ID of pet to return
    */
    pub async fn get_pet_by_id<'a>(
        &'a self,
        pet_id: i64,
    ) -> Result<ResponseValue<types::Pet>, Error<()>> {
        let url = format!("{}/pet/{}", self.baseurl, encode_path(&pet_id.to_string()),);
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
            operation_id: "get_pet_by_id",
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
    /**Updates a pet in the store with form data

    Updates a pet resource based on the form data.

    Sends a `POST` request to `/pet/{petId}`

    Arguments:
    - `pet_id`: ID of pet that needs to be updated
    - `name`: Name of pet that needs to be updated
    - `status`: Status of pet that needs to be updated
    */
    pub async fn update_pet_with_form<'a>(
        &'a self,
        pet_id: i64,
        name: Option<&'a str>,
        status: Option<&'a str>,
    ) -> Result<ResponseValue<types::Pet>, Error<()>> {
        let url = format!("{}/pet/{}", self.baseurl, encode_path(&pet_id.to_string()),);
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
            .query(&progenitor_client::QueryParam::new("name", &name))
            .query(&progenitor_client::QueryParam::new("status", &status))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "update_pet_with_form",
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
    /**Deletes a pet

    Delete a pet.

    Sends a `DELETE` request to `/pet/{petId}`

    Arguments:
    - `pet_id`: Pet id to delete
    - `api_key`:
    */
    pub async fn delete_pet<'a>(
        &'a self,
        pet_id: i64,
        api_key: Option<&'a str>,
    ) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!("{}/pet/{}", self.baseurl, encode_path(&pet_id.to_string()),);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(2usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        if let Some(value) = api_key {
            header_map.append("api_key", value.to_string().try_into()?);
        }
        #[allow(unused_mut)]
        let mut request = self.client.delete(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "delete_pet",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::empty(response)),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Uploads an image

    Upload image of the pet.

    Sends a `POST` request to `/pet/{petId}/uploadImage`

    Arguments:
    - `pet_id`: ID of pet to update
    - `additional_metadata`: Additional Metadata
    - `body`
    */
    pub async fn upload_file<'a, B: Into<reqwest::Body>>(
        &'a self,
        pet_id: i64,
        additional_metadata: Option<&'a str>,
        body: B,
    ) -> Result<ResponseValue<types::ApiResponse>, Error<()>> {
        let url = format!(
            "{}/pet/{}/uploadImage",
            self.baseurl,
            encode_path(&pet_id.to_string()),
        );
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
            .header(
                ::reqwest::header::CONTENT_TYPE,
                ::reqwest::header::HeaderValue::from_static("application/octet-stream"),
            )
            .body(body)
            .query(&progenitor_client::QueryParam::new(
                "additionalMetadata",
                &additional_metadata,
            ))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "upload_file",
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
    /**Returns pet inventories by status

    Returns a map of status codes to quantities.

    Sends a `GET` request to `/store/inventory`

    */
    pub async fn get_inventory<'a>(
        &'a self,
    ) -> Result<ResponseValue<::std::collections::HashMap<::std::string::String, i32>>, Error<()>>
    {
        let url = format!("{}/store/inventory", self.baseurl,);
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
            operation_id: "get_inventory",
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
    /**Place an order for a pet

    Place a new order in the store.

    Sends a `POST` request to `/store/order`

    */
    pub async fn place_order<'a>(
        &'a self,
        body: &'a types::Order,
    ) -> Result<ResponseValue<types::Order>, Error<()>> {
        let url = format!("{}/store/order", self.baseurl,);
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
            operation_id: "place_order",
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
    /**Find purchase order by ID

    For valid response try integer IDs with value <= 5 or > 10. Other values will generate exceptions.

    Sends a `GET` request to `/store/order/{orderId}`

    Arguments:
    - `order_id`: ID of order that needs to be fetched
    */
    pub async fn get_order_by_id<'a>(
        &'a self,
        order_id: i64,
    ) -> Result<ResponseValue<types::Order>, Error<()>> {
        let url = format!(
            "{}/store/order/{}",
            self.baseurl,
            encode_path(&order_id.to_string()),
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
            operation_id: "get_order_by_id",
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
    /**Delete purchase order by identifier

    For valid response try integer IDs with value < 1000. Anything above 1000 or non-integers will generate API errors.

    Sends a `DELETE` request to `/store/order/{orderId}`

    Arguments:
    - `order_id`: ID of the order that needs to be deleted
    */
    pub async fn delete_order<'a>(&'a self, order_id: i64) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!(
            "{}/store/order/{}",
            self.baseurl,
            encode_path(&order_id.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.delete(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "delete_order",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::empty(response)),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Create user

    This can only be done by the logged in user.

    Sends a `POST` request to `/user`

    Arguments:
    - `body`: Created user object
    */
    pub async fn create_user<'a>(
        &'a self,
        body: &'a types::User,
    ) -> Result<ResponseValue<types::User>, Error<()>> {
        let url = format!("{}/user", self.baseurl,);
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
            operation_id: "create_user",
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
    /**Creates list of users with given input array

    Creates list of users with given input array.

    Sends a `POST` request to `/user/createWithList`

    */
    pub async fn create_users_with_list_input<'a>(
        &'a self,
        body: &'a ::std::vec::Vec<types::User>,
    ) -> Result<ResponseValue<types::User>, Error<()>> {
        let url = format!("{}/user/createWithList", self.baseurl,);
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
            operation_id: "create_users_with_list_input",
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
    /**Logs user into the system

    Log into the system.

    Sends a `GET` request to `/user/login`

    Arguments:
    - `password`: The password for login in clear text
    - `username`: The user name for login
    */
    pub async fn login_user<'a>(
        &'a self,
        password: Option<&'a str>,
        username: Option<&'a str>,
    ) -> Result<ResponseValue<::std::string::String>, Error<()>> {
        let url = format!("{}/user/login", self.baseurl,);
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
            .query(&progenitor_client::QueryParam::new("password", &password))
            .query(&progenitor_client::QueryParam::new("username", &username))
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "login_user",
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
    /**Logs out current logged in user session

    Log user out of the system.

    Sends a `GET` request to `/user/logout`

    */
    pub async fn logout_user<'a>(&'a self) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!("{}/user/logout", self.baseurl,);
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.get(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "logout_user",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::empty(response)),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Get user by user name

    Get user detail based on username.

    Sends a `GET` request to `/user/{username}`

    Arguments:
    - `username`: The name that needs to be fetched. Use user1 for testing
    */
    pub async fn get_user_by_name<'a>(
        &'a self,
        username: &'a str,
    ) -> Result<ResponseValue<types::User>, Error<()>> {
        let url = format!(
            "{}/user/{}",
            self.baseurl,
            encode_path(&username.to_string()),
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
            operation_id: "get_user_by_name",
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
    /**Update user resource

    This can only be done by the logged in user.

    Sends a `PUT` request to `/user/{username}`

    Arguments:
    - `username`: name that need to be deleted
    - `body`: Update an existent user in the store
    */
    pub async fn update_user<'a>(
        &'a self,
        username: &'a str,
        body: &'a types::User,
    ) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!(
            "{}/user/{}",
            self.baseurl,
            encode_path(&username.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self
            .client
            .put(url)
            .json(&body)
            .headers(header_map)
            .build()?;
        let info = OperationInfo {
            operation_id: "update_user",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::empty(response)),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
    /**Delete user resource

    This can only be done by the logged in user.

    Sends a `DELETE` request to `/user/{username}`

    Arguments:
    - `username`: The name that needs to be deleted
    */
    pub async fn delete_user<'a>(
        &'a self,
        username: &'a str,
    ) -> Result<ResponseValue<()>, Error<()>> {
        let url = format!(
            "{}/user/{}",
            self.baseurl,
            encode_path(&username.to_string()),
        );
        let mut header_map = ::reqwest::header::HeaderMap::with_capacity(1usize);
        header_map.append(
            ::reqwest::header::HeaderName::from_static("api-version"),
            ::reqwest::header::HeaderValue::from_static(Self::api_version()),
        );
        #[allow(unused_mut)]
        let mut request = self.client.delete(url).headers(header_map).build()?;
        let info = OperationInfo {
            operation_id: "delete_user",
        };
        self.pre(&mut request, &info).await?;
        let result = self.exec(request, &info).await;
        self.post(&result, &info).await?;
        let response = result?;
        match response.status().as_u16() {
            200u16 => Ok(ResponseValue::empty(response)),
            _ => Err(Error::UnexpectedResponse(response)),
        }
    }
}
/// Items consumers will typically use such as the Client.
pub mod prelude {
    #[allow(unused_imports)]
    pub use super::Client;
}
