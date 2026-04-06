use crate::client::*;
use aomi_sdk::*;
use serde_json::Value;

// ============================================================================
// Tool 1: Search -- semantic search across Web3 corpus
// ============================================================================

impl DynAomiTool for KaitoSearch {
    type App = KaitoApp;
    type Args = KaitoSearchArgs;
    const NAME: &'static str = "kaito_search";
    const DESCRIPTION: &'static str = "Semantic search across Kaito's Web3 corpus (Twitter, Discord, Telegram, governance forums, Farcaster, podcasts, conference transcripts). Returns AI-structured results with attention quantification.";

    fn run(_app: &KaitoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = KaitoClient::new(&args.api_key)?;
        client.search(&args.query, args.limit, args.source_type.as_deref())
    }
}

// ============================================================================
// Tool 2: GetTrending -- trending topics / narratives
// ============================================================================

impl DynAomiTool for KaitoGetTrending {
    type App = KaitoApp;
    type Args = KaitoGetTrendingArgs;
    const NAME: &'static str = "kaito_get_trending";
    const DESCRIPTION: &'static str = "Get trending topics and narratives across Web3 sources from Kaito. Shows what the crypto community is currently discussing.";

    fn run(_app: &KaitoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = KaitoClient::new(&args.api_key)?;
        client.get_trending(args.limit)
    }
}

// ============================================================================
// Tool 3: GetMindshare -- token attention metrics
// ============================================================================

impl DynAomiTool for KaitoGetMindshare {
    type App = KaitoApp;
    type Args = KaitoGetMindshareArgs;
    const NAME: &'static str = "kaito_get_mindshare";
    const DESCRIPTION: &'static str = "Get attention and mindshare metrics for a specific token from Kaito. Quantifies how much discussion and attention a token is receiving across Web3 sources.";

    fn run(_app: &KaitoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = KaitoClient::new(&args.api_key)?;
        client.get_mindshare(&args.token)
    }
}
