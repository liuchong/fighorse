//! High-level Figma design package for AI replication workflows.
//!
//! High-level Figma design package for AI replication workflows.

use crate::api::files as files_api;
use crate::error::{Error, Result};
use crate::experience::{self, Filters, ScopeOpts};
use crate::figma;
use crate::guidance;
use crate::transform::{compact, tokens};
use crate::url as figma_url;
use serde_json::{json, Map, Value};

/// Resolved source coordinates for a design package request.
pub struct Source {
    pub figma_url: Option<String>,
    pub file_key: Option<String>,
    pub node_id: Option<String>,
}

impl Source {
    fn to_json(&self) -> Value {
        // dissoc :parsed — emit figma_url (if present), file_key, node_id.
        let mut m = Map::new();
        if let Some(u) = &self.figma_url {
            m.insert("figma_url".into(), Value::String(u.clone()));
        }
        m.insert(
            "file_key".into(),
            self.file_key
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        m.insert(
            "node_id".into(),
            self.node_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        Value::Object(m)
    }
}

/// Resolve a source from figma_url / file_key / node_id.
pub fn resolve_source(
    figma_url_input: Option<&str>,
    file_key: Option<&str>,
    node_id: Option<&str>,
) -> Source {
    let parsed = figma_url_input.map(figma_url::parse_figma_url);
    let resolved_file_key = file_key
        .map(String::from)
        .or_else(|| parsed.as_ref().and_then(|p| p.file_key.clone()));
    let resolved_node_id = node_id
        .map(String::from)
        .or_else(|| parsed.as_ref().and_then(|p| p.node_id.clone()));
    Source {
        figma_url: figma_url_input.map(String::from),
        file_key: resolved_file_key,
        node_id: resolved_node_id,
    }
}

fn normalize_target(value: Option<&str>, default: &str) -> String {
    match value
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
    {
        Some(v) => v,
        None => default.to_string(),
    }
}

fn missing_target(value: Option<&str>) -> bool {
    value.map(|v| v.trim().is_empty()).unwrap_or(true)
}

fn render_asset_format(asset_format: Option<&str>) -> String {
    let af = normalize_target(asset_format, "png");
    match af.as_str() {
        "jpeg" => "jpg".to_string(),
        "png" | "jpg" | "svg" | "pdf" => af,
        _ => "png".to_string(),
    }
}

fn str_contains(haystack: &str, needle: &str) -> bool {
    haystack.contains(needle)
}

fn implementation_target(platform: Option<&str>, asset_format: Option<&str>) -> Value {
    let platform = normalize_target(platform, "unspecified");
    let asset_format_missing = missing_target(asset_format);
    let asset_format_v = normalize_target(asset_format, "png");
    let render_format = render_asset_format(asset_format);

    let mut ask = Vec::new();
    if platform == "unspecified" {
        ask.push(Value::String("Target platform/framework is not specified. Ask whether the output should be Android Compose, iOS SwiftUI/UIKit, Web, Flutter, React Native, etc.".into()));
    }
    if asset_format_missing {
        ask.push(Value::String("Asset export format was not specified. Ask whether slices should be png, jpg, svg, pdf, or app-specific vector assets before exporting.".into()));
    }
    if render_format != asset_format_v {
        ask.push(Value::String(format!("Requested asset format `{asset_format_v}` is not supported by Figma node rendering. Use `{render_format}` for rendered slices, or download original image fills when the source content type provides that format.")));
    }

    let mut rules = vec![
        Value::String("Do not silently choose a different platform or asset format than the developer requested.".into()),
        Value::String("If Figma metadata is insufficient for a platform-specific decision, ask the developer instead of inventing a rule.".into()),
        Value::String("Record platform assumptions in the implementation notes or generated code comments when they affect fidelity.".into()),
    ];
    if str_contains(&platform, "android") {
        rules.push(Value::String("For Android, prefer density-independent layout values and export bitmap slices as png unless vector/svg is explicitly requested.".into()));
        rules.push(Value::String("For Jetpack Compose, map textStyle fontSize/lineHeight/fontWeight explicitly and avoid relying on default Material typography.".into()));
    }
    if str_contains(&platform, "ios") {
        rules.push(Value::String("For iOS, ask whether SwiftUI or UIKit is required and prefer pdf/svg vector assets when the app pipeline supports them.".into()));
        rules.push(Value::String("Map Figma textStyle to explicit font, size, weight, and line height instead of relying on platform defaults.".into()));
    }
    if str_contains(&platform, "web") || str_contains(&platform, "react") {
        rules.push(Value::String("For web, prefer svg for vector icons and png/webp for raster imagery according to the target build pipeline.".into()));
        rules.push(Value::String("Map Figma textStyle to CSS font-family, font-size, font-weight, line-height, and letter-spacing.".into()));
    }

    json!({
        "platform": platform,
        "asset_format": asset_format_v,
        "render_asset_format": render_format,
        "ask_developer_when": ask,
        "rules": rules,
    })
}

fn fidelity_workflow(platform: Option<&str>, asset_format: Option<&str>) -> Value {
    json!({
        "goal": "Turn Figma data into a high-fidelity implementation through iterative visual verification.",
        "platform": normalize_target(platform, "unspecified"),
        "asset_format": normalize_target(asset_format, "png"),
        "steps": [
            "Start with the whole target frame to understand screen hierarchy and major spacing.",
            "Inspect key child nodes/components for textStyle, dimensions, fills, strokes, effects, and layout constraints.",
            "Export local slices for ambiguous icons, images, controls, cards, and repeated components before approximating them.",
            "Implement the screen using explicit typography, spacing, and asset references from Figma context/tokens.",
            "Build and run the target app when possible, capture an app screenshot, and compare it against the Figma screenshot.",
            "Fix overlap, clipping, wrong font size/line-height, wrong component stacking, and status/navigation bar conflicts before moving on.",
            "Repeat the loop for representative screens and component states."
        ],
        "attention_checks": [
            "Check whether containers with multiple children are implemented with the correct layout primitive instead of stacking children accidentally.",
            "Check compact cards, message bubbles, and list rows separately from full-size versions; they often use different typography and line height.",
            "Check real device/system chrome against Figma status bars and navigation bars to avoid double-rendered bars.",
            "Check long localized strings and generated names for clipping or overlap.",
            "If a visual mismatch cannot be resolved from the package, ask the developer which tradeoff is acceptable."
        ]
    })
}

fn asset_export_plan(
    file_key: Option<&str>,
    node_id: Option<&str>,
    platform: Option<&str>,
    asset_format: Option<&str>,
) -> Value {
    let asset_format_v = normalize_target(asset_format, "png");
    let render_format = render_asset_format(asset_format);
    let platform_v = normalize_target(platform, "unspecified");
    let fk = file_key.unwrap_or("");
    let nid = node_id.unwrap_or("<node-id>");

    let mut cli_examples = vec![
        Value::String(format!("fighorse image export {fk} --ids {nid} --format {render_format} --dir ./.fighorse/exports --manifest")),
        Value::String(format!("fighorse component export {fk} --ids <component-node-id> --format {render_format} --dir ./assets/fighorse --manifest")),
        Value::String(format!("fighorse asset download {fk} --dir ./assets/fighorse --manifest")),
    ];
    if platform_v == "unspecified" {
        cli_examples.push(Value::String("Ask the developer for the target platform before choosing final asset format or density rules.".into()));
    }

    json!({
        "preferred_format": asset_format_v,
        "render_format": render_format,
        "platform": platform_v,
        "output_policy": guidance::output_location_guidance(),
        "cli_examples": cli_examples,
        "mcp_tools": [
            {"tool": "export_images", "when": "Need local frame/node slices for implementation or visual comparison."},
            {"tool": "export_component", "when": "Need a local image for a Figma component/control node."},
            {"tool": "download_image_fills", "when": "Need original raster image fills from the Figma file."}
        ]
    })
}

fn node_bounds(node: &Value) -> Value {
    node.get("absoluteBoundingBox")
        .filter(|v| !v.is_null())
        .or_else(|| node.get("absoluteRenderBounds").filter(|v| !v.is_null()))
        .or_else(|| node.get("size").filter(|v| !v.is_null()))
        .cloned()
        .unwrap_or(Value::Null)
}

/// Pre-order tree traversal (matches `tree-seq`).
fn all_nodes(node: &Value) -> Vec<Value> {
    let mut out = vec![node.clone()];
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            out.extend(all_nodes(child));
        }
    }
    out
}

fn is_blank(s: Option<&str>) -> bool {
    s.map(|v| v.trim().is_empty()).unwrap_or(true)
}

fn screen_candidates(target: &Value) -> Value {
    let out: Vec<Value> = all_nodes(target)
        .into_iter()
        .filter(|n| {
            matches!(
                n.get("type").and_then(|v| v.as_str()),
                Some("FRAME" | "COMPONENT" | "INSTANCE" | "SECTION")
            )
        })
        .filter(|n| !is_blank(n.get("id").and_then(|v| v.as_str())))
        .take(20)
        .map(|node| {
            let bounds = node_bounds(&node);
            let w = bounds.get("width").and_then(|v| v.as_f64());
            let h = bounds.get("height").and_then(|v| v.as_f64());
            let reason =
                if w.map(|w| w > 200.0).unwrap_or(false) && h.map(|h| h > 200.0).unwrap_or(false) {
                    "large candidate frame/component"
                } else {
                    "structural candidate"
                };
            json!({
                "id": node.get("id").cloned().unwrap_or(Value::Null),
                "name": node.get("name").cloned().unwrap_or(Value::Null),
                "type": node.get("type").cloned().unwrap_or(Value::Null),
                "width": bounds.get("width").cloned().unwrap_or(Value::Null),
                "height": bounds.get("height").cloned().unwrap_or(Value::Null),
                "renderable": node.get("id").map(|v| !v.is_null()).unwrap_or(false),
                "reason": reason,
            })
        })
        .collect();
    Value::Array(out)
}

fn component_candidates(target: &Value) -> Value {
    let out: Vec<Value> = all_nodes(target)
        .into_iter()
        .filter(|n| {
            matches!(
                n.get("type").and_then(|v| v.as_str()),
                Some("COMPONENT" | "COMPONENT_SET" | "INSTANCE")
            )
        })
        .take(30)
        .map(|node| {
            json!({
                "id": node.get("id").cloned().unwrap_or(Value::Null),
                "name": node.get("name").cloned().unwrap_or(Value::Null),
                "type": node.get("type").cloned().unwrap_or(Value::Null),
                "component_id": node.get("componentId").cloned().unwrap_or(Value::Null),
                "component_set_id": node.get("componentSetId").cloned().unwrap_or(Value::Null),
                "bounds": node_bounds(&node),
            })
        })
        .collect();
    Value::Array(out)
}

fn missing_font_diagnostics(target: &Value) -> Value {
    let text_nodes: Vec<Value> = all_nodes(target)
        .into_iter()
        .filter(|n| n.get("type").and_then(|v| v.as_str()) == Some("TEXT"))
        .collect();
    let missing: Vec<Value> = text_nodes
        .iter()
        .filter(|n| {
            is_blank(
                n.get("style")
                    .and_then(|s| s.get("fontFamily"))
                    .and_then(|v| v.as_str()),
            )
        })
        .take(20)
        .map(|n| {
            json!({
                "id": n.get("id").cloned().unwrap_or(Value::Null),
                "name": n.get("name").cloned().unwrap_or(Value::Null),
                "type": n.get("type").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    let has_missing = !missing.is_empty();
    json!({
        "checked_text_nodes": text_nodes.len(),
        "missing_font_family_count": missing.len(),
        "examples": missing,
        "ai_guidance": if has_missing {
            "Some text nodes do not expose fontFamily in the compact package. Inspect raw node details or ask the developer about font availability before approximating typography."
        } else {
            "Font family metadata is present for inspected text nodes."
        }
    })
}

fn truncated(node: &Value) -> bool {
    if node.get("truncated") == Some(&Value::Bool(true)) {
        return true;
    }
    node.get("children")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().any(truncated))
        .unwrap_or(false)
}

fn token_counts(grouped: &Value) -> Map<String, Value> {
    let mut counts = Map::new();
    if let Some(obj) = grouped.as_object() {
        for (k, v) in obj {
            let count = v.as_array().map(|a| a.len()).unwrap_or(0);
            counts.insert(k.clone(), json!(count));
        }
    }
    counts
}

fn total_tokens(counts: &Map<String, Value>) -> i64 {
    counts.values().filter_map(|v| v.as_i64()).sum()
}

fn token_confidence(grouped: &Value) -> Value {
    let counts = token_counts(grouped);
    let total = total_tokens(&counts);
    let status = if total == 0 {
        "missing"
    } else if total < 4 {
        "low"
    } else {
        "usable"
    };
    json!({
        "status": status,
        "counts": Value::Object(counts),
        "ai_guidance": "Use token values when present, but verify typography and raster assets against screenshots."
    })
}

fn file_summary(data: &Value) -> Value {
    let keys = [
        "name",
        "lastModified",
        "thumbnailUrl",
        "version",
        "role",
        "editorType",
        "linkAccess",
    ];
    let mut m = Map::new();
    if let Some(obj) = data.as_object() {
        for k in keys {
            if let Some(v) = obj.get(k) {
                m.insert(k.to_string(), v.clone());
            }
        }
    }
    Value::Object(m)
}

fn implementation_risk_checklist(
    target: &Value,
    platform: Option<&str>,
    asset_format: Option<&str>,
) -> Value {
    let mut checklist = vec![
        Value::String("Check selected target scope before coding; CANVAS or flow nodes should be narrowed to frames.".into()),
        Value::String("Check screenshot fidelity after implementation, not only structured JSON.".into()),
        Value::String("Check repeated siblings, compact cards, and list rows for overlap or wrong layout primitives.".into()),
        Value::String("Check asset export manifest before referencing local files.".into()),
    ];
    if is_blank(platform) {
        checklist.push(Value::String("Target platform is unspecified; ask before choosing framework, density, or native controls.".into()));
    }
    if is_blank(asset_format) {
        checklist.push(Value::String(
            "Asset format is unspecified; ask before exporting final slices.".into(),
        ));
    }
    if target.get("type").and_then(|v| v.as_str()) == Some("CANVAS") {
        checklist.push(Value::String(
            "Current target is a CANVAS/page; use screen_candidates to pick exact frames.".into(),
        ));
    }
    Value::Array(checklist)
}

fn implementation_hints(platform: Option<&str>, asset_format: Option<&str>) -> Value {
    json!({
        "intent": "Use this package to recreate the selected Figma design in code.",
        "priority_order": ["screenshots", "learned_experience", "tokens", "context", "assets", "file metadata"],
        "layout": "Use context.children, dimensions, layout, padding, itemSpacing, and textStyle to build component hierarchy.",
        "visuals": "Use tokens and node fills/strokes/effects for colors, borders, shadows, typography, and spacing.",
        "assets": "Use screenshots for verification and image_fills for raster assets when present.",
        "platform": "Use implementation_target.platform when present. If it is unspecified, ask the developer before choosing a platform/framework.",
        "asset_format": "Use implementation_target.asset_format for exported slices/assets. If the app pipeline requires another format, ask before converting.",
        "output_locations": "Use asset_export_plan.output_policy. Prefer ./.fighorse/exports for temporary slices and ./assets/fighorse or app resource directories for packaged assets. Ask before writing elsewhere.",
        "ai_contract": guidance::ai_contract(),
        "quality_checks": [
            "Compare final UI against screenshots.",
            "Preserve visible text content.",
            "If target.type is CANVAS or the target has many children, narrow to exact frame/screen nodes before implementing.",
            "Map font family, font size, line height, font weight, and letter spacing explicitly.",
            "Check compact components separately from full-size components.",
            "Run the app and capture screenshots when possible; fix overlap/clipping from real screenshots.",
            "Prefer exact token values over approximate colors.",
            "If context is truncated, request a lower-depth package or targeted node package.",
            "Export ambiguous assets with manifest enabled into a reasonable project-local or fighorse-managed directory.",
            "If platform, asset format, or fidelity tradeoffs are unclear, ask the developer instead of guessing."
        ],
        "target": implementation_target(platform, asset_format)
    })
}

fn assets_image_count(assets: &Value) -> usize {
    assets
        .get("meta")
        .and_then(|m| m.get("images"))
        .or_else(|| assets.get("images"))
        .and_then(|v| v.as_object())
        .map(|o| o.len())
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
fn diagnostics(
    target: &Value,
    compacted: &Value,
    grouped_tokens: &Value,
    screenshots: &Option<Value>,
    assets: &Option<Value>,
    platform: Option<&str>,
    asset_format: Option<&str>,
) -> Value {
    let image_count = screenshots
        .as_ref()
        .and_then(|s| s.get("images"))
        .and_then(|v| v.as_object())
        .map(|o| o.len())
        .unwrap_or(0);
    let asset_count = assets.as_ref().map(assets_image_count).unwrap_or(0);
    let counts = token_counts(grouped_tokens);
    let token_total = total_tokens(&counts);

    let platform_v = normalize_target(platform, "unspecified");
    let asset_format_missing = missing_target(asset_format);
    let asset_format_v = normalize_target(asset_format, "png");
    let render_format = render_asset_format(asset_format);
    let target_type = target.get("type").and_then(|v| v.as_str());
    let child_count = target
        .get("children")
        .and_then(|c| c.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let is_truncated = truncated(compacted);

    let mut warnings: Vec<Value> = Vec::new();
    if image_count == 0 {
        warnings.push(Value::String(
            "No screenshot URL was returned. Use get_screenshot or lower the target scope.".into(),
        ));
    }
    if token_total == 0 {
        warnings.push(Value::String(
            "No design tokens were extracted. Inspect context fills/textStyle directly.".into(),
        ));
    }
    if is_truncated {
        warnings.push(Value::String("Context was token-budget truncated. Request a smaller node or larger max_tokens for more detail.".into()));
    }
    if platform_v == "unspecified" {
        warnings.push(Value::String("Target platform/framework was not specified. Ask the developer before choosing platform-specific typography, density, or asset rules.".into()));
    }
    if asset_format_missing {
        warnings.push(Value::String("Asset export format was not specified. Ask the developer before choosing final slice or asset format.".into()));
    }
    if render_format != asset_format_v {
        warnings.push(Value::String(format!("Asset format `{asset_format_v}` is not supported for rendered node slices. Use `{render_format}` for render exports, and use image fill downloads for original raster formats when available.")));
    }
    if target_type == Some("CANVAS") {
        warnings.push(Value::String("Selected target is a CANVAS/page. Ask for the exact frame/screen node or inspect child frames before implementing.".into()));
    }
    if child_count > 12 {
        warnings.push(Value::String("Selected target has many direct children. Treat it as a flow/overview and narrow to representative frames before coding.".into()));
    }

    let categories: Vec<Value> = counts.keys().map(|k| Value::String(k.clone())).collect();

    json!({
        "status": if image_count > 0 && token_total > 0 { "ready" } else { "partial" },
        "context_truncated": is_truncated,
        "screenshots": {"requested": screenshots.is_some(), "count": image_count},
        "tokens": {"categories": categories, "counts": Value::Object(counts)},
        "assets": {"requested": assets.is_some(), "count": asset_count},
        "warnings": warnings,
        "production_note": "Use status=ready as the normal AI replication path; handle status=partial by following warnings and next_tools."
    })
}

#[allow(clippy::too_many_arguments)]
fn package_base(
    source: &Source,
    data: &Value,
    target: &Value,
    compacted: &Value,
    grouped_tokens: &Value,
    screenshots: &Option<Value>,
    assets: &Option<Value>,
    platform: Option<&str>,
    asset_format: Option<&str>,
) -> Value {
    let filters = Filters {
        platform: platform.map(String::from),
        asset_format: asset_format.map(String::from),
        ..Default::default()
    };
    let learned = experience::guidance(&filters, 6, &ScopeOpts::default());

    let mut diag = diagnostics(
        target,
        compacted,
        grouped_tokens,
        screenshots,
        assets,
        platform,
        asset_format,
    );
    if let Some(obj) = diag.as_object_mut() {
        obj.insert(
            "experience".into(),
            json!({
                "store_path": learned.get("summary").and_then(|s| s.get("store_path")).cloned().unwrap_or(Value::Null),
                "schema_version": experience::SCHEMA_VERSION,
                "total_count": learned.get("summary").and_then(|s| s.get("total_count")).cloned().unwrap_or(Value::Null),
                "returned_count": learned.get("summary").and_then(|s| s.get("returned_count")).cloned().unwrap_or(Value::Null),
                "next_step": "Review learned_experience before implementation and call record_experience after reusable fixes."
            }),
        );
    }

    json!({
        "kind": "fighorse.design-package.v1",
        "source": source.to_json(),
        "file": file_summary(data),
        "target": figma::node_summary(target),
        "implementation_target": implementation_target(platform, asset_format),
        "screen_candidates": screen_candidates(target),
        "component_candidates": component_candidates(target),
        "ai_contract": guidance::ai_contract(),
        "fidelity_workflow": fidelity_workflow(platform, asset_format),
        "asset_export_plan": asset_export_plan(source.file_key.as_deref(), source.node_id.as_deref(), platform, asset_format),
        "learned_experience": learned,
        "context": compacted,
        "tokens": grouped_tokens,
        "token_confidence": token_confidence(grouped_tokens),
        "missing_font_diagnostics": missing_font_diagnostics(target),
        "implementation_risk_checklist": implementation_risk_checklist(target, platform, asset_format),
        "diagnostics": diag,
        "implementation_hints": implementation_hints(platform, asset_format),
        "next_tools": [
            {"tool": "list_experiences", "when": "Need local lessons before implementing or when a mismatch repeats."},
            {"tool": "get_screenshot", "when": "Need a fresh render URL or a different scale/format."},
            {"tool": "export_images", "when": "Need local frame/node slices and a manifest for app implementation."},
            {"tool": "export_component", "when": "Need to export a control/component node as an image slice."},
            {"tool": "download_image_fills", "when": "Need original image fill assets with usable file extensions."},
            {"tool": "record_experience", "when": "After fixing a reusable overlap, clipping, typography, asset, platform, or workflow issue."},
            {"tool": "get_tokens", "when": "Need a token-only payload."},
            {"tool": "get_design_context", "when": "Need compact context without screenshots/assets."}
        ]
    })
}

/// Options for a design package request.
pub struct PackageOpts<'a> {
    pub figma_url: Option<&'a str>,
    pub file_key: Option<&'a str>,
    pub node_id: Option<&'a str>,
    pub depth: i64,
    pub max_tokens: i64,
    pub include_screenshot: bool,
    pub include_assets: bool,
    pub screenshot_format: String,
    pub scale: f64,
    pub screenshot_limit: usize,
    pub platform: Option<&'a str>,
    pub asset_format: Option<&'a str>,
}

fn num_str(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Build a full design package by fetching Figma data and assembling context.
pub async fn get_design_package(token: &str, opts: PackageOpts<'_>) -> Result<Value> {
    let source = resolve_source(opts.figma_url, opts.file_key, opts.node_id);
    if source
        .file_key
        .as_deref()
        .map(|k| k.is_empty())
        .unwrap_or(true)
    {
        return Err(Error::Other("A Figma URL or file_key is required".into()));
    }
    let file_key = source.file_key.clone().unwrap();
    let node_id = source.node_id.clone();
    let depth_str = opts.depth.to_string();

    let data = if let Some(nid) = &node_id {
        if !nid.is_empty() {
            files_api::get_file_nodes(token, &file_key, nid, None, Some(&depth_str), None, None)
                .await?
        } else {
            files_api::get_file(
                token,
                &file_key,
                files_api::GetFileParams {
                    depth: Some(&depth_str),
                    ..Default::default()
                },
            )
            .await?
        }
    } else {
        files_api::get_file(
            token,
            &file_key,
            files_api::GetFileParams {
                depth: Some(&depth_str),
                ..Default::default()
            },
        )
        .await?
    };

    let target = figma::response_to_node(&data);
    let simplified = compact::simplify_tree(&target, Some(opts.depth));
    let compacted = compact::compact(&target, Some(opts.depth), Some(opts.max_tokens));
    let grouped_tokens = tokens::tokens_by_category(&tokens::extract_tokens(&simplified));

    let screenshot_ids =
        figma::renderable_node_ids(&target, node_id.as_deref(), Some(opts.screenshot_limit));

    let screenshots: Option<Value> = if opts.include_screenshot && !screenshot_ids.is_empty() {
        let joined = screenshot_ids.join(",");
        Some(
            files_api::get_images(
                token,
                &file_key,
                &joined,
                None,
                Some(&num_str(opts.scale)),
                Some(&opts.screenshot_format),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?,
        )
    } else {
        None
    };

    let assets: Option<Value> = if opts.include_assets {
        Some(files_api::get_image_fills(token, &file_key).await?)
    } else {
        None
    };

    let mut package = package_base(
        &source,
        &data,
        &target,
        &compacted,
        &grouped_tokens,
        &screenshots,
        &assets,
        opts.platform,
        opts.asset_format,
    );

    if let Some(obj) = package.as_object_mut() {
        if let Some(shots) = &screenshots {
            obj.insert(
                "screenshots".into(),
                json!({
                    "format": opts.screenshot_format,
                    "scale": num_str(opts.scale).parse::<f64>().unwrap_or(opts.scale),
                    "node_ids": screenshot_ids,
                    "images": shots.get("images").cloned().unwrap_or(Value::Null),
                }),
            );
        }
        if let Some(a) = &assets {
            obj.insert(
                "assets".into(),
                json!({"image_fills": a.get("meta").cloned().unwrap_or(Value::Null)}),
            );
        }
    }

    Ok(package)
}
