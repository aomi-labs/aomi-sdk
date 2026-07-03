use crate::client::Client as GenClient;
#[allow(unused_imports)]
use crate::client::types::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub(crate) struct E2eDemoApp;

const BASE_URL: &str = "https://petstore3.swagger.io/api/v3";
const DEFAULT_PET_PHOTO_URL: &str = "https://example.com/pet.png";

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value).map_err(|e| format!("[e2e-demo] serialize: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".into(), Value::String("e2e-demo".into()));
            Value::Object(map)
        }
        other => json!({ "source": "e2e-demo", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[e2e-demo] runtime: {e}"))
}

fn build_client() -> GenClient {
    GenClient::new(BASE_URL)
}

fn parse_enum<T>(value: Option<&str>, field: &str) -> Result<Option<T>, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|e| format!("[e2e-demo] invalid {field}: {e}"))
        })
        .transpose()
}

fn build_tags(tags: &[String]) -> Vec<Tag> {
    tags.iter()
        .filter_map(|tag| {
            let tag = tag.trim();
            if tag.is_empty() {
                None
            } else {
                Some(Tag {
                    id: None,
                    name: Some(tag.to_string()),
                })
            }
        })
        .collect()
}

fn build_pet(
    name: &str,
    status: Option<&str>,
    category: Option<&str>,
    tags: &[String],
) -> Result<Pet, String> {
    Ok(Pet {
        category: category.map(|name| Category {
            id: None,
            name: Some(name.to_string()),
        }),
        id: None,
        name: name.to_string(),
        photo_urls: vec![DEFAULT_PET_PHOTO_URL.to_string()],
        status: parse_enum::<PetStatus>(status, "pet status")?,
        tags: build_tags(tags),
    })
}

fn build_user(
    username: &str,
    password: &str,
    email: Option<&str>,
    first_name: Option<&str>,
    last_name: Option<&str>,
) -> User {
    User {
        email: email.map(|value| value.to_string()),
        first_name: first_name.map(|value| value.to_string()),
        id: None,
        last_name: last_name.map(|value| value.to_string()),
        password: Some(password.to_string()),
        phone: None,
        user_status: None,
        username: Some(username.to_string()),
    }
}

fn build_order(pet_id: i64, quantity: Option<i32>, status: Option<&str>) -> Result<Order, String> {
    Ok(Order {
        complete: None,
        id: None,
        pet_id: Some(pet_id),
        quantity,
        ship_date: None,
        status: parse_enum::<OrderStatus>(status, "order status")?,
    })
}

// ============================================================================
// Tool 1: CreatePet
// ============================================================================

pub(crate) struct CreatePet;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CreatePetArgs {
    /// Pet name to create.
    pub(crate) name: String,
    /// Optional pet status. Use `available`, `pending`, or `sold`.
    pub(crate) status: Option<String>,
    /// Optional category label to attach to the pet.
    pub(crate) category: Option<String>,
    /// Optional tag names. Empty strings are ignored.
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

impl DynAomiTool for CreatePet {
    type App = E2eDemoApp;
    type Args = CreatePetArgs;
    const NAME: &'static str = "e2e_demo_create_pet";
    const DESCRIPTION: &'static str = "Use when the user wants to seed a new demo pet and immediately confirm the API stored it. Creates the pet with a default photo URL, then fetches it back by ID so the caller can validate the round-trip.";

    fn run(_app: &E2eDemoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = build_client();
            let pet = build_pet(
                args.name.as_str(),
                args.status.as_deref(),
                args.category.as_deref(),
                &args.tags,
            )?;
            let created = client
                .add_pet(&pet)
                .await
                .map_err(|e| format!("[e2e-demo] add_pet: {e}"))?
                .into_inner();
            let pet_id = created
                .id
                .ok_or_else(|| "[e2e-demo] add_pet returned no pet id".to_string())?;
            let verified = client
                .get_pet_by_id(pet_id)
                .await
                .map_err(|e| format!("[e2e-demo] get_pet_by_id: {e}"))?
                .into_inner();
            ok(json!({
                "created": created,
                "verified": verified,
            }))
        })
    }
}

// ============================================================================
// Tool 2: UpdatePet
// ============================================================================

pub(crate) struct UpdatePet;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct UpdatePetArgs {
    /// ID of the pet to update.
    pub(crate) pet_id: i64,
    /// New pet name, if you want to rename the pet.
    pub(crate) name: Option<String>,
    /// New pet status. Use `available`, `pending`, or `sold`.
    pub(crate) status: Option<String>,
}

impl DynAomiTool for UpdatePet {
    type App = E2eDemoApp;
    type Args = UpdatePetArgs;
    const NAME: &'static str = "e2e_demo_update_pet";
    const DESCRIPTION: &'static str = "Use when the user wants to rename a demo pet or change its store status and confirm the stored record. Sends the form update, then fetches the pet by ID to validate the change.";

    fn run(_app: &E2eDemoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        if args.name.is_none() && args.status.is_none() {
            return Err("[e2e-demo] update_pet requires name and/or status".to_string());
        }

        let runtime = rt()?;
        runtime.block_on(async move {
            let client = build_client();
            let updated = client
                .update_pet_with_form(args.pet_id, args.name.as_deref(), args.status.as_deref())
                .await
                .map_err(|e| format!("[e2e-demo] update_pet_with_form: {e}"))?
                .into_inner();
            let verified = client
                .get_pet_by_id(args.pet_id)
                .await
                .map_err(|e| format!("[e2e-demo] get_pet_by_id: {e}"))?
                .into_inner();
            ok(json!({
                "updated": updated,
                "verified": verified,
            }))
        })
    }
}

// ============================================================================
// Tool 3: FindPets
// ============================================================================

pub(crate) struct FindPets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FindPetsArgs {
    /// Filter by pet status when you are not searching by tags.
    pub(crate) status: Option<String>,
    /// Optional tag names. When present, tag search takes precedence over status.
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

impl DynAomiTool for FindPets {
    type App = E2eDemoApp;
    type Args = FindPetsArgs;
    const NAME: &'static str = "e2e_demo_find_pets";
    const DESCRIPTION: &'static str = "Use when the user wants to inspect the current demo inventory before or after a write. Searches by status by default, or by tag list when tags are provided.";

    fn run(_app: &E2eDemoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = build_client();
            if !args.tags.is_empty() {
                let pets = client
                    .find_pets_by_tags(&args.tags)
                    .await
                    .map_err(|e| format!("[e2e-demo] find_pets_by_tags: {e}"))?
                    .into_inner();
                return ok(json!({
                    "mode": "tags",
                    "tags": args.tags,
                    "pets": pets,
                }));
            }

            let status =
                match parse_enum::<FindPetsByStatusStatus>(args.status.as_deref(), "pet status")? {
                    Some(status) => status,
                    None => FindPetsByStatusStatus::Available,
                };
            let pets = client
                .find_pets_by_status(status)
                .await
                .map_err(|e| format!("[e2e-demo] find_pets_by_status: {e}"))?
                .into_inner();
            ok(json!({
                "mode": "status",
                "status": status,
                "pets": pets,
            }))
        })
    }
}

// ============================================================================
// Tool 4: CreateUserAndLogin
// ============================================================================

pub(crate) struct CreateUserAndLogin;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CreateUserAndLoginArgs {
    /// Username to create.
    pub(crate) username: String,
    /// Password to store and reuse for login.
    pub(crate) password: String,
    /// Optional email address.
    pub(crate) email: Option<String>,
    /// Optional first name.
    pub(crate) first_name: Option<String>,
    /// Optional last name.
    pub(crate) last_name: Option<String>,
}

impl DynAomiTool for CreateUserAndLogin {
    type App = E2eDemoApp;
    type Args = CreateUserAndLoginArgs;
    const NAME: &'static str = "e2e_demo_create_user_and_login";
    const DESCRIPTION: &'static str = "Use when the user wants to provision a demo account and validate the auth path. Creates the user, logs in with the same credentials, and fetches the profile back by username.";

    fn run(_app: &E2eDemoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = build_client();
            let user = build_user(
                args.username.as_str(),
                args.password.as_str(),
                args.email.as_deref(),
                args.first_name.as_deref(),
                args.last_name.as_deref(),
            );
            let created = client
                .create_user(&user)
                .await
                .map_err(|e| format!("[e2e-demo] create_user: {e}"))?
                .into_inner();
            let login = client
                .login_user(Some(args.password.as_str()), Some(args.username.as_str()))
                .await
                .map_err(|e| format!("[e2e-demo] login_user: {e}"))?
                .into_inner();
            let verified = client
                .get_user_by_name(args.username.as_str())
                .await
                .map_err(|e| format!("[e2e-demo] get_user_by_name: {e}"))?
                .into_inner();
            ok(json!({
                "created": created,
                "login": login,
                "verified": verified,
            }))
        })
    }
}

// ============================================================================
// Tool 5: GetUser
// ============================================================================

pub(crate) struct GetUser;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetUserArgs {
    /// Username to look up.
    pub(crate) username: String,
}

impl DynAomiTool for GetUser {
    type App = E2eDemoApp;
    type Args = GetUserArgs;
    const NAME: &'static str = "e2e_demo_get_user";
    const DESCRIPTION: &'static str = "Use when the user wants to inspect an existing demo account by username without mutating anything.";

    fn run(_app: &E2eDemoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = build_client();
            let user = client
                .get_user_by_name(args.username.as_str())
                .await
                .map_err(|e| format!("[e2e-demo] get_user_by_name: {e}"))?
                .into_inner();
            ok(user)
        })
    }
}

// ============================================================================
// Tool 6: PlaceOrder
// ============================================================================

pub(crate) struct PlaceOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceOrderArgs {
    /// The pet ID to order.
    pub(crate) pet_id: i64,
    /// Quantity to order. Defaults to 1 when omitted.
    pub(crate) quantity: Option<i32>,
    /// Optional order status. Use `placed`, `approved`, or `delivered`.
    pub(crate) status: Option<String>,
}

impl DynAomiTool for PlaceOrder {
    type App = E2eDemoApp;
    type Args = PlaceOrderArgs;
    const NAME: &'static str = "e2e_demo_place_order";
    const DESCRIPTION: &'static str = "Use when the user wants to place a demo store order for a pet. Creates the order with sensible defaults and returns the resulting order object.";

    fn run(_app: &E2eDemoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = build_client();
            let order = build_order(args.pet_id, args.quantity, args.status.as_deref())?;
            let created = client
                .place_order(&order)
                .await
                .map_err(|e| format!("[e2e-demo] place_order: {e}"))?
                .into_inner();
            ok(created)
        })
    }
}
