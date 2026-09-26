//! Bounded, headless structural checks. Executing/lowering legacy kit code
//! belongs to the separate runtime validator, never the Hub service process.
use crate::gate::{Finding, MAX_BUNDLE_BYTES};
use serde::Serialize;
use serde_json::Value;
use std::{fs, io::Read, path::{Component, Path, PathBuf}};

pub const MAX_ENTRIES: usize = 2048;
pub const MAX_DEPTH: usize = 32;
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
pub const MAX_TEXT_BYTES: u64 = 1024 * 1024;
pub const MAX_IMAGE_DIMENSION: u32 = 4096;

#[derive(Clone, Debug)]
pub struct BundleFile { pub path: PathBuf, pub bytes: u64 }

/// Inspect metadata before reading or allocating payloads. Empty directories
/// count toward the same traversal limit as files.
pub fn inventory(root: &Path) -> Result<Vec<BundleFile>, String> {
    fn walk(root: &Path, dir: &Path, depth: usize, entries: &mut usize, remaining: &mut u64, out: &mut Vec<BundleFile>) -> Result<(), String> {
        if depth > MAX_DEPTH { return Err("bundle exceeds directory depth limit".into()); }
        if !fs::symlink_metadata(dir).map_err(|e| e.to_string())?.is_dir() {
            return Err("bundle directories must be real directories, not symlinks".into());
        }
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            *entries += 1;
            if *entries > MAX_ENTRIES { return Err("bundle exceeds file count limit".into()); }
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
            let name = relative.to_str().ok_or("bundle paths must be UTF-8")?;
            safe_relative(name)?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() { walk(root, &path, depth + 1, entries, remaining, out)?; }
            else if kind.is_file() {
                let bytes = entry.metadata().map_err(|e| e.to_string())?.len();
                if relative == Path::new("manifest.json") {
                    if bytes > MAX_MANIFEST_BYTES { return Err("manifest.json exceeds size limit".into()); }
                } else {
                    *remaining = remaining.checked_sub(bytes).ok_or("bundle exceeds size limit")?;
                }
                out.push(BundleFile { path: relative.into(), bytes });
            } else { return Err(format!("{name}: only regular files and directories are allowed")); }
        }
        Ok(())
    }
    let mut files = Vec::new();
    let mut entries = 0;
    let mut remaining = MAX_BUNDLE_BYTES;
    walk(root, root, 0, &mut entries, &mut remaining, &mut files)?;
    files.sort_by(|a,b| a.path.cmp(&b.path));
    Ok(files)
}

pub fn safe_relative(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains(['\\', ':', '\0']) || name.split('/').any(|s| s.is_empty() || s == "." || s == "..")
        || Path::new(name).components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(format!("not a portable bundle path: {name:?}"));
    }
    Ok(())
}

pub fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() || meta.len() > max { return Err(format!("{}: not a regular file within the size limit", path.display())); }
    let mut bytes = Vec::new();
    fs::File::open(path).map_err(|e| e.to_string())?.take(max + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max { return Err(format!("{}: exceeds size limit", path.display())); }
    Ok(bytes)
}

pub fn read_text(path: &Path, max: u64) -> Result<String, String> {
    String::from_utf8(read_bounded(path, max)?).map_err(|e| format!("{}: invalid UTF-8: {e}", path.display()))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind { Image, Font, SvgReference, DisplayUrl }

#[derive(Clone, Debug, Serialize)]
pub struct ResourceReference { pub path: String, pub target: String, pub kind: ResourceKind }

/// JSON pointers identify the property a developer needs to fix. Display URLs
/// are recorded separately; the existing conservative text rule still applies.
fn resources(value: &Value, path: &str, out: &mut Vec<ResourceReference>, rendered: bool) {
    match value {
        Value::Object(object) => for (key, value) in object {
            let pointer = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
            if let Some(target) = value.as_str() {
                let kind = match key.as_str() {
                    "src" | "image" if rendered => Some(ResourceKind::Image),
                    "font_src" if rendered => Some(ResourceKind::Font),
                    _ if target.contains("https://") || target.contains("http://") => Some(ResourceKind::DisplayUrl),
                    _ => None,
                };
                if let Some(kind) = kind { out.push(ResourceReference { path: pointer.clone(), target: target.into(), kind }); }
            }
            resources(value, &pointer, out, rendered);
        },
        Value::Array(items) => for (index, value) in items.iter().enumerate() { resources(value, &format!("{path}/{index}"), out, rendered); },
        _ => {},
    }
}

pub fn validate(root: &Path, files: &[BundleFile]) -> (Vec<Finding>, Vec<ResourceReference>) {
    let mut findings = Vec::new();
    let mut refs = Vec::new();
    let icon = read_text(&root.join("listing.json"), MAX_TEXT_BYTES).ok()
        .and_then(|text| octosense_app_policy::Listing::parse(&text).ok()).and_then(|listing| listing.icon);
    for file in files {
        let name = file.path.to_string_lossy();
        let path = root.join(&file.path);
        let extension = file.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let result = match extension.as_str() {
            "card" | "json" | "l0" | "octoscript" | "splash" | "txt" | "md" => read_text(&path, MAX_TEXT_BYTES).and_then(|text| {
                if extension == "json" {
                    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
                    if name != "manifest.json" && name != "listing.json" {
                        // Arbitrary app data and kit prop declarations are not
                        // resource requests. Only known renderer fields are.
                        resources(&value, &name, &mut refs, false);
                        if name == "page.data.json" {
                            resources(&value["$kit"]["placements"], "page.data.json/$kit/placements", &mut refs, true);
                        } else if name.starts_with("kit/native/") && name.ends_with("/kit.json") {
                            if let Some(components) = value["components"].as_object() {
                                for (component, spec) in components {
                                    resources(&spec["style"], &format!("{name}/components/{component}/style"), &mut refs, true);
                                }
                            }
                        }
                    }
                }
                Ok(())
            }),
            "png" | "jpg" | "jpeg" | "webp" => validate_bitmap(&path, &extension, icon.as_deref() == Some(name.as_ref())),
            "svg" => validate_svg(&path, &name, &mut refs, icon.as_deref() == Some(name.as_ref())),
            _ => Ok(()),
        };
        if let Err(error) = result { findings.push(Finding::at("contents-invalid", name.as_ref(), error)); }
    }
    if files.iter().any(|file| file.path == Path::new(octosense_app_policy::SCRIPT_ENTRY)) {
        if let Err(error) = read_text(&root.join(octosense_app_policy::SCRIPT_ENTRY), MAX_TEXT_BYTES) {
            findings.push(Finding::at("script-invalid", octosense_app_policy::SCRIPT_ENTRY, error));
        }
    } else if let Err((path, error)) = validate_card(root) {
        findings.push(Finding::at("card-invalid", path, error));
    }
    for reference in &refs {
        if matches!(reference.kind, ResourceKind::DisplayUrl) { continue; }
        if matches!(reference.kind, ResourceKind::Font) && reference.target == "makepad_widgets:resources/Inter.ttf" { continue; }
        if matches!(reference.kind, ResourceKind::SvgReference) && reference.target.starts_with('#') { continue; }
        let result = safe_relative(&reference.target).and_then(|_| {
            if files.iter().any(|f| f.path == Path::new(&reference.target)) { Ok(()) }
            else { Err(format!("missing bundled resource {}", reference.target)) }
        });
        if let Err(error) = result { findings.push(Finding::at("resource-invalid", &reference.path, error)); }
    }
    (findings, refs)
}

fn validate_card(root: &Path) -> Result<(), (String, String)> {
    let card = read_text(&root.join("page.card"), octoscript_ui_l0::DEFAULT_MAX_SOURCE_BYTES as u64).map_err(|e| ("page.card".into(), e))?;
    let data: Value = if root.join("page.data.json").exists() {
        serde_json::from_str(&read_text(&root.join("page.data.json"), MAX_TEXT_BYTES).map_err(|e| ("page.data.json".into(), e))?)
            .map_err(|e| ("page.data.json".into(), e.to_string()))?
    } else { serde_json::json!({}) };
    let report = octoscript_ui_l0::check_ui_l0(&card);
    if !report.valid {
        return Err(("page.card".into(), report.diagnostics.iter().map(|d| format!("{}:{} {}", d.line, d.column, d.message)).collect::<Vec<_>>().join("; ")));
    }
    let mood = octoscript_ui_l0::card_theme(&card).unwrap_or_else(|| "dark".into());
    if root.join(format!("kit/native/{mood}/kit.json")).is_file() {
        let name = format!("kit/native/{mood}/kit.json");
        safe_relative(&name).map_err(|e| ("page.card".into(), e))?;
        let pack: Value = serde_json::from_str(&read_text(&root.join(&name), MAX_TEXT_BYTES).map_err(|e| (name.clone(), e))?)
            .map_err(|e| (name.clone(), e.to_string()))?;
        if pack["theme"] != mood { return Err((name, "kit theme does not match the card".into())); }
        // A native pack can also be a legacy Card's optional theme overlay.
        // Its presence does not select the rendering mode or require placements.
        if data["$kit"]["placements"].is_object() {
            check_native_pack(&pack, &data).map_err(|e| (name, e))?;
        }
        // Realization and kit expansion are intentionally deferred to the
        // resource-limited validator process: input bounds do not bound expansion.
    } else {
        // Legacy kits contain executable Octoscript. Check closure here;
        // evaluate it only inside the bounded runtime validator process.
        let mut names = vec!["_palette_dark.octoscript".into(), "_derive_color.octoscript".into(), "_derive.octoscript".into(), "_kit.octoscript".into()];
        if mood != "dark" { names.push(format!("_palette_{mood}.octoscript")); }
        for (axis, value) in octoscript_ui_l0::card_theme_axes(&card) {
            if matches!(value.as_str(), "neutral" | "regular" | "none" | "soft" | "sans") { continue; }
            names.push(if axis == "accent" { format!("_axis_accent_{value}_{mood}.octoscript") } else { format!("_axis_{axis}_{value}.octoscript") });
        }
        for name in names {
            let name = format!("kit/{name}");
            safe_relative(&name).and_then(|_| read_text(&root.join(&name), MAX_TEXT_BYTES).map(|_| ())).map_err(|e| (name, e))?;
        }
    }
    Ok(())
}

fn check_native_pack(pack: &Value, data: &Value) -> Result<(), String> {
    if pack["schema_version"] != 1 { return Err("unsupported kit schema".into()); }
    let components = pack["components"].as_object().ok_or("missing kit components")?;
    let placements = data["$kit"]["placements"].as_object().ok_or("missing kit placements")?;
    // Count retained JSON memory conservatively without cloning token values.
    fn weight(value: &Value) -> u64 {
        64 + match value {
            Value::String(s) => s.len() as u64,
            Value::Array(a) => a.iter().map(weight).sum(),
            Value::Object(o) => o.iter().map(|(k,v)| k.len() as u64 + 64 + weight(v)).sum(),
            _ => 0,
        }
    }
    // Each token is traversed once even when thousands of styles reuse it.
    let token_weights: std::collections::BTreeMap<&str, u64> = pack["tokens"].as_object().into_iter()
        .flat_map(|tokens| tokens.iter()).filter_map(|(name, token)| token.get("value").map(|value| (name.as_str(), weight(value)))).collect();
    let mut costs = std::collections::BTreeMap::new();
    for (name, component) in components {
        let style = component["style"].as_object().ok_or("missing kit component style")?;
        let mut bytes = 0u64;
        for value in style.values() {
            bytes += if let Some(token) = value.get("$token").and_then(Value::as_str) {
                *token_weights.get(token).ok_or_else(|| format!("missing kit token {token}"))?
            } else { weight(value) };
            if bytes > 8 * 1024 * 1024 { return Err("kit expansion exceeds structural size limit".into()); }
        }
        costs.insert(name.as_str(), bytes);
    }
    let mut total = 0u64;
    for placement in placements.values() {
        let name = placement["component"].as_str().ok_or("missing kit placement component")?;
        total += costs.get(name).ok_or_else(|| format!("unknown kit component {name}"))? + weight(placement);
        if total > 8 * 1024 * 1024 { return Err("kit expansion exceeds structural size limit".into()); }
    }
    Ok(())
}

fn validate_bitmap(path: &Path, extension: &str, square: bool) -> Result<(), String> {
    let bytes = read_bounded(path, if square { 1024 * 1024 } else { MAX_BUNDLE_BYTES })?;
    let format = image::ImageFormat::from_extension(extension).ok_or("unsupported image extension")?;
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    let dimension = if square { 1024 } else { MAX_IMAGE_DIMENSION };
    limits.max_image_width = Some(dimension);
    limits.max_image_height = Some(dimension);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    use image::ImageDecoder;
    let decoder = reader.into_decoder().map_err(|e| format!("cannot decode image: {e}"))?;
    if decoder.total_bytes() > 64 * 1024 * 1024 { return Err("decoded image exceeds size limit".into()); }
    let (width, height) = decoder.dimensions();
    if square && width != height { return Err("launcher icon must be square".into()); }
    image::DynamicImage::from_decoder(decoder).map_err(|e| format!("cannot decode image: {e}"))?;
    Ok(())
}

fn validate_svg(path: &Path, name: &str, refs: &mut Vec<ResourceReference>, square: bool) -> Result<(), String> {
    let text = read_text(path, MAX_TEXT_BYTES)?;
    let document = roxmltree::Document::parse_with_options(&text, roxmltree::ParsingOptions { nodes_limit: 4096, ..Default::default() }).map_err(|e| e.to_string())?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" { return Err("expected SVG document".into()); }
    let view_box: Vec<f64> = root.attribute("viewBox").unwrap_or("").split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
    let mut dimensions = Vec::new();
    for (attr, offset) in [("width", 2), ("height", 3)] {
        let value = root.attribute(attr).and_then(|s| s.trim_end_matches("px").parse::<f64>().ok()).or_else(|| view_box.get(offset).copied()).ok_or("SVG needs numeric dimensions or a viewBox")?;
        dimensions.push(value);
        if !value.is_finite() || value <= 0. || value > f64::from(MAX_IMAGE_DIMENSION) { return Err("SVG dimensions exceed limits".into()); }
    }
    if square && dimensions[0] != dimensions[1] { return Err("launcher icon must be square".into()); }
    for node in root.descendants().filter(|n| n.is_element()) {
        if matches!(node.tag_name().name(), "script" | "foreignObject" | "style") { return Err("SVG executable or external styling is not supported".into()); }
        for attr in node.attributes() {
            if attr.name().starts_with("on") || attr.value().contains("@import") { return Err("SVG active content is not supported".into()); }
            if attr.name() == "href" {
                refs.push(ResourceReference { path: format!("{name}/{}@href", node.tag_name().name()), target: attr.value().into(), kind: ResourceKind::SvgReference });
            }
            // CSS escapes and comments can obscure URL tokens. This supported
            // subset refuses them, then checks every case-insensitive url().
            let lower = attr.value().to_ascii_lowercase();
            if lower.contains('\\') || lower.contains("/*") { return Err("escaped SVG styling is not supported".into()); }
            for (_, tail) in lower.match_indices("url").map(|(at, token)| (token, &lower[at+3..])) {
                let tail = tail.trim_start();
                if let Some(tail) = tail.strip_prefix('(') {
                    let end = tail.find(')').ok_or("unterminated SVG resource URL")?;
                    let target = tail[..end].trim().trim_matches(['\'', '"']);
                    if !target.starts_with('#') || target.len() == 1 || target.contains(char::is_whitespace) {
                        return Err("SVG resource URLs must be local fragment references".into());
                    }
                }
            }
        }
    }
    Ok(())
}
