//! Product-layer visual audit guidance for Figma implementation loops.
//!
//! Product-layer visual audit guidance for Figma implementation loops.

use crate::experience::{self, Filters, ScopeOpts};
use crate::guidance;
use crate::url as figma_url;
use serde_json::{json, Value};

/// Build a visual audit payload.
pub fn audit(
    figma_url_input: Option<&str>,
    screenshot_path: Option<&str>,
    platform: Option<&str>,
    asset_format: Option<&str>,
    notes: Option<&str>,
) -> Value {
    let parsed = figma_url_input
        .filter(|u| !u.trim().is_empty())
        .map(figma_url::parse_figma_url);
    let filters = Filters {
        platform: platform.map(String::from),
        asset_format: asset_format.map(String::from),
        ..Default::default()
    };
    let learned = experience::guidance(&filters, 6, &ScopeOpts::default());

    json!({
        "kind": "fighorse.visual-audit.v1",
        "source": {
            "figma_url": figma_url_input,
            "parsed": parsed.as_ref().map(|p| p.to_json()),
            "implementation_screenshot": screenshot_path
        },
        "target": {
            "platform": platform.unwrap_or("unspecified"),
            "asset_format": asset_format.unwrap_or("unspecified")
        },
        "status": if screenshot_path.map(|s| s.trim().is_empty()).unwrap_or(true) {
            "checklist-only"
        } else {
            "ready-for-human-or-ai-comparison"
        },
        "checklist": [
            "Compare the implementation screenshot against the Figma render at the same viewport or device size.",
            "Check frame bounds, safe areas, status bars, navigation bars, and scroll behavior first.",
            "Check typography explicitly: font family, size, weight, line height, and letter spacing.",
            "Check repeated rows/cards for accidental stacking, overlap, clipping, or missing list containers.",
            "Check icons, raster fills, and component states against exported assets and manifests.",
            "Check long text and localization-sensitive labels for wrapping and overflow.",
            "Record reusable findings with record_experience or fighorse experience add."
        ],
        "ai_guidance": {
            "prompt": "Use this audit checklist to produce a concise mismatch report: observed difference, likely cause, fix, and whether it should become reusable experience.",
            "next_tools": ["get_design_package", "export_images", "download_image_fills", "record_experience"]
        },
        "learned_experience": learned,
        "notes": notes,
        "output_policy": guidance::output_location_guidance()
    })
}
