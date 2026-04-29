use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct EmptyRequest {}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_z: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dx: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dz: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExploreRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_z: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateObjectRequest {
    pub prompt: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CustomizeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PingRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
}
