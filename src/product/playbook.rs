//! Project-level AI playbook assembled from fighorse guidance and experience.
//!
//! Project-level AI playbook assembled from guidance and local experience.

use crate::api::coverage;
use crate::experience::{self, Filters, ScopeOpts};
use crate::guidance;
use serde_json::{json, Value};

/// Build the project playbook payload.
pub fn build(platform: Option<&str>, asset_format: Option<&str>, project_dir: Option<&str>) -> Value {
    let filters = Filters {
        platform: platform.map(String::from),
        asset_format: asset_format.map(String::from),
        ..Default::default()
    };
    let opts = ScopeOpts {
        project_dir: project_dir.map(String::from),
        ..Default::default()
    };
    let learned = experience::guidance(&filters, 12, &opts);

    json!({
        "kind": "fighorse.project-playbook.v1",
        "target": {
            "platform": platform.unwrap_or("unspecified"),
            "asset_format": asset_format.unwrap_or("unspecified"),
            "project_dir": project_dir
        },
        "principles": [
            "Use fighorse discovery before low-level tools.",
            "Use design packages for implementation context and screenshots.",
            "Export ambiguous assets with manifests into approved project-local roots.",
            "Run the target app and compare screenshots before declaring fidelity complete.",
            "Record transferable lessons after fixing visual, asset, or platform issues."
        ],
        "official_api": {
            "coverage": coverage::operation_summary(),
            "generic_cli": "fighorse figma api <operationId> --params '{...}'",
            "generic_mcp": "Use figma_<operation_id_in_snake_case> tools for exact REST operations."
        },
        "ai_contract": guidance::ai_contract(),
        "output_policy": guidance::output_location_guidance(),
        "learned_experience": learned,
        "next_steps": [
            "Call get_design_package for the target Figma URL.",
            "Call visual_audit after capturing an implementation screenshot.",
            "Call record_experience with reusable lessons."
        ]
    })
}
