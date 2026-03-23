use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};

impl DynAomiTool for MolinarGetState {
    type App = MolinarApp;
    type Args = MolinarGetStateArgs;
    const NAME: &'static str = "molinar_get_state";
    const DESCRIPTION: &'static str = "Get the current world snapshot without taking any action: your position, nearby players, coins, buildings, terrain, and chat history. Use to poll the world between decisions.";

    fn run(_app: &MolinarApp, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.get_state(&bot_id)
    }
}

// ============================================================================
// Tool 2: Look Around
// ============================================================================

pub(crate) struct MolinarLook;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarLookArgs {}

impl DynAomiTool for MolinarLook {
    type App = MolinarApp;
    type Args = MolinarLookArgs;
    const NAME: &'static str = "molinar_look";
    const DESCRIPTION: &'static str = "Look around — returns the full world snapshot (same as get_state). Use when you want to observe your surroundings.";

    fn run(_app: &MolinarApp, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.look(&bot_id)
    }
}

// ============================================================================
// Tool 3: Move Character
// ============================================================================

pub(crate) struct MolinarMove;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarMoveArgs {
    /// Direction to move (required) — moves exactly 1 tile per call.
    /// Valid: forward, backward, left, right, forward_left, forward_right,
    /// backward_left, backward_right, north, south, east, west.
    direction: String,
}

impl DynAomiTool for MolinarMove {
    type App = MolinarApp;
    type Args = MolinarMoveArgs;
    const NAME: &'static str = "molinar_move";
    const DESCRIPTION: &'static str = "Move the character exactly 1 tile in the specified direction. Call multiple times to travel further distances. Valid directions: forward, backward, left, right, forward_left, forward_right, backward_left, backward_right, north, south, east, west.";

    fn run(_app: &MolinarApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        let payload = json!({
            "direction": args.direction
        });
        client.move_bot(&bot_id, payload)
    }
}

// ============================================================================
// Tool 4: Jump
// ============================================================================

pub(crate) struct MolinarJump;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarJumpArgs {}

impl DynAomiTool for MolinarJump {
    type App = MolinarApp;
    type Args = MolinarJumpArgs;
    const NAME: &'static str = "molinar_jump";
    const DESCRIPTION: &'static str =
        "Make the character jump. Only works when grounded. Returns action.jumped (true/false).";

    fn run(_app: &MolinarApp, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.jump(&bot_id)
    }
}

// ============================================================================
// Tool 5: Send Chat Message
// ============================================================================

pub(crate) struct MolinarChat;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarChatArgs {
    /// Text message to send to all players (max 200 chars)
    message: String,
}

impl DynAomiTool for MolinarChat {
    type App = MolinarApp;
    type Args = MolinarChatArgs;
    const NAME: &'static str = "molinar_chat";
    const DESCRIPTION: &'static str = "Send a chat message visible to all players in the world. Returns updated world state with chat history.";

    fn run(_app: &MolinarApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.send_chat(&bot_id, &args.message)
    }
}

// ============================================================================
// Tool 6: Get Chat History
// ============================================================================

pub(crate) struct MolinarGetChat;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarGetChatArgs {}

impl DynAomiTool for MolinarGetChat {
    type App = MolinarApp;
    type Args = MolinarGetChatArgs;
    const NAME: &'static str = "molinar_get_chat";
    const DESCRIPTION: &'static str = "Get chat history (last 20 messages). Returns full world snapshot with chat in the `chat` field.";

    fn run(_app: &MolinarApp, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.get_chat(&bot_id)
    }
}

// ============================================================================
// Tool 7: Get New Messages
// ============================================================================

pub(crate) struct MolinarGetNewMessages;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarGetNewMessagesArgs {}

impl DynAomiTool for MolinarGetNewMessages {
    type App = MolinarApp;
    type Args = MolinarGetNewMessagesArgs;
    const NAME: &'static str = "molinar_get_new_messages";
    const DESCRIPTION: &'static str = "Get new chat messages from other players since the last check. Messages are drained (returned once). Use to detect when someone talks to you. New messages in action.newMessages.";

    fn run(_app: &MolinarApp, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.get_new_messages(&bot_id)
    }
}

// ============================================================================
// Tool 8: Get Players
// ============================================================================

pub(crate) struct MolinarGetPlayers;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarGetPlayersArgs {}

impl DynAomiTool for MolinarGetPlayers {
    type App = MolinarApp;
    type Args = MolinarGetPlayersArgs;
    const NAME: &'static str = "molinar_get_players";
    const DESCRIPTION: &'static str = "Get all online players. Returns full world snapshot — all players in `allPlayers`, nearby ones in `nearby.players` with distance and direction.";

    fn run(_app: &MolinarApp, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.get_players(&bot_id)
    }
}

// ============================================================================
// Tool 9: Collect Coins
// ============================================================================

pub(crate) struct MolinarCollectCoins;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarCollectCoinsArgs {}

impl DynAomiTool for MolinarCollectCoins {
    type App = MolinarApp;
    type Args = MolinarCollectCoinsArgs;
    const NAME: &'static str = "molinar_collect_coins";
    const DESCRIPTION: &'static str = "Collect nearby coins. If coins are within reach, collects them. Otherwise moves towards the nearest coin. Call repeatedly to farm. Gold=1pt, Silver=2pt, Gem=5pt. Returns action.collected count and action.movingTo.";

    fn run(_app: &MolinarApp, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.collect_coins(&bot_id)
    }
}

// ============================================================================
// Tool 10: Explore
// ============================================================================

pub(crate) struct MolinarExplore;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarExploreArgs {}

impl DynAomiTool for MolinarExplore {
    type App = MolinarApp;
    type Args = MolinarExploreArgs;
    const NAME: &'static str = "molinar_explore";
    const DESCRIPTION: &'static str = "Explore the world by stepping 1 tile in a random cardinal direction. Call multiple times to wander and discover new areas. Returns updated world state.";

    fn run(_app: &MolinarApp, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.explore(&bot_id, json!({}))
    }
}

// ============================================================================
// Tool 11: Create 3D Object
// ============================================================================

pub(crate) struct MolinarCreateObject;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarCreateObjectArgs {
    /// Description of the 3D object to AI-generate and place at current position
    /// (e.g. "a giant golden mushroom house")
    prompt: String,
}

impl DynAomiTool for MolinarCreateObject {
    type App = MolinarApp;
    type Args = MolinarCreateObjectArgs;
    const NAME: &'static str = "molinar_create_object";
    const DESCRIPTION: &'static str = "AI-generate a 3D object from a text description and place it at the bot's current position in the world.";

    fn run(_app: &MolinarApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        client.create_object(&bot_id, &args.prompt)
    }
}

// ============================================================================
// Tool 12: Customize Appearance
// ============================================================================

pub(crate) struct MolinarCustomize;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarCustomizeArgs {
    /// New display name (max 20 chars)
    name: Option<String>,
    /// New color from palette: white (#ffffff), light-blue (#a8d8ea),
    /// pink (#ffb7c5), yellow (#ffe66d), mint (#98d8c8), peach (#ffc49c),
    /// red (#ff6b6b), purple (#aa96da), blue (#3498db), green (#2ecc71),
    /// gold (#ffd700), black (#2b2b2b), lavender (#e6e6fa)
    color: Option<String>,
}

impl DynAomiTool for MolinarCustomize {
    type App = MolinarApp;
    type Args = MolinarCustomizeArgs;
    const NAME: &'static str = "molinar_customize";
    const DESCRIPTION: &'static str =
        "Change the character's display name and/or color. Color must be from the allowed palette.";

    fn run(_app: &MolinarApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        let mut payload = json!({});
        if let Some(name) = &args.name {
            payload["name"] = Value::String(name.clone());
        }
        if let Some(color) = &args.color {
            payload["color"] = Value::String(color.clone());
        }
        client.customize(&bot_id, payload)
    }
}

// ============================================================================
// Tool 13: Ping Location
// ============================================================================

pub(crate) struct MolinarPing;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarPingArgs {
    /// X coordinate to ping (default: current position)
    x: Option<f64>,
    /// Z coordinate to ping (default: current position)
    z: Option<f64>,
}

impl DynAomiTool for MolinarPing {
    type App = MolinarApp;
    type Args = MolinarPingArgs;
    const NAME: &'static str = "molinar_ping";
    const DESCRIPTION: &'static str = "Send a visual ping marker that other players can see. Defaults to current position if no coordinates provided.";

    fn run(_app: &MolinarApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let bot_id = get_bot_id(&ctx)?;
        let client = MolinarClient::new()?;
        let mut payload = json!({});
        if let Some(x) = args.x {
            payload["x"] = json!(x);
        }
        if let Some(z) = args.z {
            payload["z"] = json!(z);
        }
        client.ping(&bot_id, payload)
    }
}
