use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaceOrderRequest<'a> {
    pub(crate) inst_id: &'a str,
    pub(crate) td_mode: &'a str,
    pub(crate) side: &'a str,
    pub(crate) ord_type: &'a str,
    pub(crate) sz: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) px: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CancelOrderRequest<'a> {
    pub(crate) inst_id: &'a str,
    pub(crate) ord_id: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetLeverageRequest<'a> {
    pub(crate) inst_id: &'a str,
    pub(crate) lever: &'a str,
    pub(crate) mgn_mode: &'a str,
}
