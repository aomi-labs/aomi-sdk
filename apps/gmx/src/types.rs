use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AccountQuery<'a> {
    pub(crate) account: &'a str,
}
