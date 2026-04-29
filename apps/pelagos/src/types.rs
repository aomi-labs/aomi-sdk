use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RpcRequest<'a, T> {
    pub(crate) jsonrpc: &'static str,
    pub(crate) id: u64,
    pub(crate) method: &'a str,
    pub(crate) params: T,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BalanceParams<'a> {
    pub(crate) user: &'a str,
    pub(crate) token: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TransferTransaction {
    pub(crate) sender: String,
    pub(crate) receiver: String,
    pub(crate) value: u64,
    pub(crate) token: String,
    pub(crate) hash: String,
}
