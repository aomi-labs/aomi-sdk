//! `aomi-build check-icon [DIR]`: checks the package folder's `[package] icon`
//! against the Library icon rule. The UI draws an icon as a CSS mask in the
//! accent colour, so the file only describes a shape: a square SVG in
//! `currentColor` that runs, fetches and declares nothing. The rules and
//! messages are the host's release check (`aomi-registry` `icon.rs`); keep
//! the two identical.

use std::fs;
use std::path::PathBuf;

use clap::Args;
use eyre::{Result, bail, eyre};

#[derive(Args, Debug)]
pub struct CheckIconArgs {
    /// The package folder holding `aomi.toml`.
    #[arg(default_value = ".")]
    pub dir: PathBuf,
}

pub fn run(args: CheckIconArgs) -> Result<()> {
    let manifest = fs::read_to_string(args.dir.join("aomi.toml"))?;
    let manifest: toml::Value = toml::from_str(&manifest)?;
    let Some(icon) = manifest
        .get("package")
        .and_then(|package| package.get("icon"))
    else {
        println!("no icon: the Library shows the generic package icon");
        return Ok(());
    };
    let icon = icon
        .as_str()
        .ok_or_else(|| eyre!("[package] icon is not a string"))?;
    let svg = fs::read(args.dir.join(icon)).map_err(|e| eyre!("icon {icon}: {e}"))?;
    if let Err(rule) = check_icon(icon, &svg) {
        bail!("icon {icon}: {rule}");
    }
    println!("icon {icon} passes the icon rule");
    Ok(())
}

/// The icon cap.
const ICON_CAP: usize = 32 * 1024;

/// The properties that carry a colour; each is `currentColor` or `none`.
const COLOURS: [&str; 6] = [
    "fill",
    "stroke",
    "color",
    "stop-color",
    "flood-color",
    "lighting-color",
];

/// Checks the icon file `name` with contents `svg`. The error names the rule.
pub fn check_icon(name: &str, svg: &[u8]) -> Result<(), String> {
    if !name.to_ascii_lowercase().ends_with(".svg") {
        return Err("the icon must be an .svg file".into());
    }
    if svg.len() > ICON_CAP {
        return Err(format!(
            "the icon is {} bytes; the limit is 32 KB",
            svg.len()
        ));
    }
    let text = std::str::from_utf8(svg).map_err(|_| "the icon is not UTF-8 text".to_string())?;
    let lower = text.to_ascii_lowercase();
    if lower.contains("<!doctype") || lower.contains("<!entity") {
        return Err("DOCTYPE and ENTITY declarations are not allowed".into());
    }
    let doc = roxmltree::Document::parse(text)
        .map_err(|_| "the icon is not well-formed XML".to_string())?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return Err("the root element must be <svg>".into());
    }
    let side = root
        .attribute("viewBox")
        .map(|view| {
            view.split([' ', ',', '\t', '\n', '\r'])
                .filter(|n| !n.is_empty())
        })
        .map(|numbers| {
            numbers
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()
        });
    if !matches!(side, Some(Ok(box_)) if box_.len() == 4 && box_[2] > 0.0 && box_[2] == box_[3]) {
        return Err("the <svg> needs a square viewBox".into());
    }
    for node in doc.descendants() {
        if let Some(pi) = node.pi() {
            return Err(format!(
                "the processing instruction <?{}?> is not allowed",
                pi.target
            ));
        }
        let element = node.tag_name().name();
        if let Some(banned) = ["script", "foreignObject", "image", "style"]
            .into_iter()
            .find(|banned| element.eq_ignore_ascii_case(banned))
        {
            return Err(format!("<{banned}> is not allowed"));
        }
        for attribute in node.attributes() {
            check_attribute(attribute.name(), attribute.value())?;
        }
    }
    Ok(())
}

/// One attribute: no event handler, nothing outside the file, no colour
/// but `currentColor`. A `style` attribute is checked declaration by
/// declaration.
fn check_attribute(name: &str, value: &str) -> Result<(), String> {
    let lower = value.to_ascii_lowercase();
    if name.to_ascii_lowercase().starts_with("on") {
        return Err(format!("the event attribute {name} is not allowed"));
    }
    if name == "href" && !value.trim_start().starts_with('#') {
        return Err("an href outside the file is not allowed".into());
    }
    if value.contains('\\') {
        return Err("a CSS escape is not allowed".into());
    }
    if lower.split("url(").skip(1).any(|rest| {
        !rest
            .trim_start()
            .trim_start_matches(['"', '\''])
            .starts_with('#')
    }) {
        return Err("a url() outside the file is not allowed".into());
    }
    let declarations: Vec<(&str, &str)> = if name == "style" {
        value
            .split(';')
            .filter_map(|declaration| declaration.split_once(':'))
            .collect()
    } else {
        vec![(name, value)]
    };
    for (property, colour) in declarations {
        let (property, colour) = (property.trim(), colour.trim());
        if COLOURS.contains(&property.to_ascii_lowercase().as_str())
            && !["currentcolor", "none"].contains(&colour.to_ascii_lowercase().as_str())
        {
            return Err(format!(
                "the colour {property}: {colour} is not allowed; use currentColor or none"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg(body: &str) -> String {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">{body}</svg>"#)
    }

    fn refusal(text: &str) -> String {
        check_icon("icon.svg", text.as_bytes()).unwrap_err()
    }

    #[test]
    fn a_monochrome_square_svg_passes() {
        let icon = r##"<?xml version="1.0" encoding="UTF-8"?>
<!-- a mark -->
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
     viewBox="0,0,16,16" fill="none" stroke="currentColor" opacity="0.9">
  <title>Mark</title>
  <defs><clipPath id="c"><rect width="16" height="16"/></clipPath><path id="p" d="M1 1h4"/></defs>
  <g clip-path="url(#c)" style="fill: currentColor; fill-opacity: .5">
    <path d="M2 2h12v12H2z" fill="CurrentColor"/>
    <use xlink:href="#p"/>
  </g>
</svg>"##;
        check_icon("Icon.SVG", icon.as_bytes()).unwrap();
        check_icon("icon.svg", svg(r#"<path d="M0 0h1"/>"#).as_bytes()).unwrap();
    }

    #[test]
    fn the_example_icons_pass() {
        let examples = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
        for example in ["hello-reader", "keyed-header"] {
            let svg = fs::read(format!("{examples}/{example}/icon.svg")).unwrap();
            check_icon("icon.svg", &svg).unwrap();
        }
    }

    #[test]
    fn the_file_is_a_small_utf8_svg() {
        let ok = svg("");
        assert_eq!(
            check_icon("icon.png", ok.as_bytes()).unwrap_err(),
            "the icon must be an .svg file"
        );
        let big = svg(&format!("<!--{}-->", "x".repeat(ICON_CAP)));
        assert_eq!(
            refusal(&big),
            format!("the icon is {} bytes; the limit is 32 KB", big.len())
        );
        assert_eq!(
            check_icon("icon.svg", b"<svg \xff/>").unwrap_err(),
            "the icon is not UTF-8 text"
        );
        assert_eq!(
            refusal("<svg viewBox='0 0 1 1'>"),
            "the icon is not well-formed XML"
        );
        assert_eq!(
            refusal("<html viewBox='0 0 1 1'/>"),
            "the root element must be <svg>"
        );
    }

    #[test]
    fn the_view_box_is_square() {
        for root in [
            "<svg/>",
            "<svg viewBox='0 0 24 12'/>",
            "<svg viewBox='0 0 0 0'/>",
            "<svg viewBox='0 0 24'/>",
            "<svg viewBox='0 0 a a'/>",
        ] {
            assert_eq!(refusal(root), "the <svg> needs a square viewBox", "{root}");
        }
    }

    #[test]
    fn declarations_are_refused() {
        for text in [
            r#"<!DOCTYPE svg><svg viewBox="0 0 1 1"/>"#,
            r#"<!doctype svg [<!ENTITY a "b">]><svg viewBox="0 0 1 1"/>"#,
        ] {
            assert_eq!(
                refusal(text),
                "DOCTYPE and ENTITY declarations are not allowed"
            );
        }
        assert_eq!(
            refusal(r#"<?xml-stylesheet href="https://x.test/a.css"?><svg viewBox="0 0 1 1"/>"#),
            "the processing instruction <?xml-stylesheet?> is not allowed"
        );
    }

    #[test]
    fn active_and_embedded_content_is_refused() {
        for (body, message) in [
            ("<script>alert(1)</script>", "<script> is not allowed"),
            ("<SCRIPT/>", "<script> is not allowed"),
            (
                r#"<h:script xmlns:h="http://www.w3.org/2000/svg"/>"#,
                "<script> is not allowed",
            ),
            ("<foreignObject/>", "<foreignObject> is not allowed"),
            (r##"<image href="#a"/>"##, "<image> is not allowed"),
            ("<style>path{fill:red}</style>", "<style> is not allowed"),
            (
                r#"<path onload="x()"/>"#,
                "the event attribute onload is not allowed",
            ),
            (
                r#"<path ONCLICK="x()"/>"#,
                "the event attribute ONCLICK is not allowed",
            ),
        ] {
            assert_eq!(refusal(&svg(body)), message, "{body}");
        }
    }

    #[test]
    fn nothing_outside_the_file_is_referenced() {
        for (body, message) in [
            (
                r#"<use href="https://x.test/a.svg#p"/>"#,
                "an href outside the file is not allowed",
            ),
            (
                r#"<a xmlns:x="http://www.w3.org/1999/xlink" x:href="javascript:x()"/>"#,
                "an href outside the file is not allowed",
            ),
            (
                r#"<g mask="url(https://x.test/m.svg)"/>"#,
                "a url() outside the file is not allowed",
            ),
            (
                r#"<g style="mask: URL( 'data:image/png,x')"/>"#,
                "a url() outside the file is not allowed",
            ),
            (
                r#"<g style="mask: \75rl(https://x.test)"/>"#,
                "a CSS escape is not allowed",
            ),
        ] {
            assert_eq!(refusal(&svg(body)), message, "{body}");
        }
    }

    #[test]
    fn only_current_color_draws() {
        for (body, message) in [
            (
                r#"<path fill="red"/>"#,
                "the colour fill: red is not allowed; use currentColor or none",
            ),
            (
                r##"<path stroke="#000"/>"##,
                "the colour stroke: #000 is not allowed; use currentColor or none",
            ),
            (
                r#"<g color="blue"/>"#,
                "the colour color: blue is not allowed; use currentColor or none",
            ),
            (
                r#"<stop stop-color="white"/>"#,
                "the colour stop-color: white is not allowed; use currentColor or none",
            ),
            (
                r#"<path style="opacity:.5;Fill: rgb(0,0,0)"/>"#,
                "the colour Fill: rgb(0,0,0) is not allowed; use currentColor or none",
            ),
            (
                r##"<path fill="url(#g)"/>"##,
                "the colour fill: url(#g) is not allowed; use currentColor or none",
            ),
            (
                r#"<path fill="&#114;ed"/>"#,
                "the colour fill: red is not allowed; use currentColor or none",
            ),
        ] {
            assert_eq!(refusal(&svg(body)), message, "{body}");
        }
    }
}
