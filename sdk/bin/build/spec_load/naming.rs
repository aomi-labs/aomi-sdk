//! Turning spec identifiers into Rust ones, plus the small spec lookups the
//! passes and both generators share.

pub fn snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            if prev_lower {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower = false;
        } else if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_lower = true;
        } else {
            if !out.ends_with('_') {
                out.push('_');
            }
            prev_lower = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// Append `_` if `s` is a Rust 2024 keyword (matches progenitor's convention).
pub fn escape_keyword(s: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "do", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod",
        "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
        "trait", "true", "try", "type", "typeof", "union", "unsafe", "unsized", "use", "virtual",
        "where", "while", "yield",
    ];
    if KEYWORDS.contains(&s) {
        format!("{s}_")
    } else {
        s.to_string()
    }
}

pub fn pascal_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper_next {
                out.push(ch.to_ascii_uppercase());
            } else {
                out.push(ch);
            }
            upper_next = false;
        } else {
            upper_next = true;
        }
    }
    out
}

pub(super) fn is_success(code: &openapiv3::StatusCode) -> bool {
    match code {
        openapiv3::StatusCode::Code(c) => (200..300).contains(c),
        openapiv3::StatusCode::Range(2) => true,
        _ => false,
    }
}

pub(super) fn synthesize_op_id(method: &str, path: &str) -> String {
    let mut s = String::from(method);
    let mut sep = true;
    for ch in path.chars() {
        match ch {
            '/' | '{' | '}' | '-' | '.' => {
                if !sep {
                    s.push('_');
                    sep = true;
                }
            }
            c if c.is_ascii_alphanumeric() => {
                s.push(c.to_ascii_lowercase());
                sep = false;
            }
            _ => {}
        }
    }
    while s.ends_with('_') {
        s.pop();
    }
    s
}

pub fn default_server_url(spec: &openapiv3::OpenAPI) -> String {
    spec.servers
        .first()
        .map(|s| s.url.clone())
        .unwrap_or_default()
}
