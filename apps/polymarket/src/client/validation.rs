pub(crate) fn normalize_yes_no(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "y" => Ok("YES".to_string()),
        "no" | "n" => Ok("NO".to_string()),
        _ => Err("outcome must be YES or NO".to_string()),
    }
}

pub(crate) fn normalize_side(value: Option<&str>) -> Result<String, String> {
    match value {
        None => Ok("BUY".to_string()),
        Some(raw) => match raw.trim().to_ascii_uppercase().as_str() {
            "BUY" => Ok("BUY".to_string()),
            "SELL" => Ok("SELL".to_string()),
            _ => Err("side must be BUY or SELL".to_string()),
        },
    }
}

pub(crate) fn validate_confirmation_token(value: Option<&str>) -> Result<(), String> {
    let Some(raw) = value else {
        return Err(
            "Missing explicit confirmation. Require confirmation='confirm' before order submission."
                .to_string(),
        );
    };
    if raw.trim().eq_ignore_ascii_case("confirm") {
        return Ok(());
    }
    Err("Invalid confirmation token. Expected confirmation='confirm'.".to_string())
}
