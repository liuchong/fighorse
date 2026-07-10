//! Figma OpenAPI coverage registry.
//!
//! Figma OpenAPI coverage registry: the explicit local contract keeping REST
//! wrappers, CLI, MCP, discovery, and tests aligned with the vendored Figma
//! OpenAPI snapshot.

use serde_json::{json, Value};

pub const OPENAPI_VERSION: &str = "0.41.0";

/// A single covered Figma REST operation.
#[derive(Debug, Clone)]
pub struct Operation {
    pub tag: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub operation_id: &'static str,
    pub deprecated: bool,
    pub policy: &'static str,
    pub status: &'static str,
}

impl Operation {
    pub fn is_write(&self) -> bool {
        self.policy == "write"
    }

    /// Base JSON shape (`select-keys` of the operation) used in envelopes.
    pub fn summary_json(&self) -> Value {
        json!({
            "operation_id": self.operation_id,
            "method": self.method,
            "path": self.path,
            "tag": self.tag,
            "deprecated": self.deprecated,
        })
    }
}

const fn op(
    tag: &'static str,
    method: &'static str,
    path: &'static str,
    operation_id: &'static str,
    deprecated: bool,
    policy: Option<&'static str>,
) -> Operation {
    let policy = match policy {
        Some(p) => p,
        None => {
            // (= "GET" method) ? "readonly" : "write"
            if matches!(method.as_bytes(), b"GET") {
                "readonly"
            } else {
                "write"
            }
        }
    };
    Operation {
        tag,
        method,
        path,
        operation_id,
        deprecated,
        policy,
        status: "covered",
    }
}

/// The full ordered list of official operations.
pub fn official_operations() -> Vec<Operation> {
    vec![
        op("Files", "GET", "/v1/files/{file_key}", "getFile", false, None),
        op("Files", "GET", "/v1/files/{file_key}/nodes", "getFileNodes", false, None),
        op("Files", "GET", "/v1/images/{file_key}", "getImages", false, None),
        op("Files", "GET", "/v1/files/{file_key}/images", "getImageFills", false, None),
        op("Files", "GET", "/v1/files/{file_key}/meta", "getFileMeta", false, None),
        op("Projects", "GET", "/v1/teams/{team_id}/projects", "getTeamProjects", false, None),
        op("Projects", "GET", "/v1/projects/{project_id}/files", "getProjectFiles", false, None),
        op("Projects", "GET", "/v1/projects/{project_id}/meta", "getProjectMeta", false, None),
        op("Files", "GET", "/v1/files/{file_key}/versions", "getFileVersions", false, None),
        op("Comments", "GET", "/v1/files/{file_key}/comments", "getComments", false, None),
        op("Comments", "POST", "/v1/files/{file_key}/comments", "postComment", false, None),
        op("Comments", "DELETE", "/v1/files/{file_key}/comments/{comment_id}", "deleteComment", false, None),
        op("Comment Reactions", "GET", "/v1/files/{file_key}/comments/{comment_id}/reactions", "getCommentReactions", false, None),
        op("Comment Reactions", "POST", "/v1/files/{file_key}/comments/{comment_id}/reactions", "postCommentReaction", false, None),
        op("Comment Reactions", "DELETE", "/v1/files/{file_key}/comments/{comment_id}/reactions", "deleteCommentReaction", false, None),
        op("Users", "GET", "/v1/me", "getMe", false, None),
        op("Components", "GET", "/v1/teams/{team_id}/components", "getTeamComponents", false, None),
        op("Components", "GET", "/v1/files/{file_key}/components", "getFileComponents", false, None),
        op("Components", "GET", "/v1/components/{key}", "getComponent", false, None),
        op("Component Sets", "GET", "/v1/teams/{team_id}/component_sets", "getTeamComponentSets", false, None),
        op("Component Sets", "GET", "/v1/files/{file_key}/component_sets", "getFileComponentSets", false, None),
        op("Component Sets", "GET", "/v1/component_sets/{key}", "getComponentSet", false, None),
        op("Styles", "GET", "/v1/teams/{team_id}/styles", "getTeamStyles", false, None),
        op("Styles", "GET", "/v1/files/{file_key}/styles", "getFileStyles", false, None),
        op("Styles", "GET", "/v1/styles/{key}", "getStyle", false, None),
        op("Webhooks", "GET", "/v2/webhooks", "getWebhooks", false, None),
        op("Webhooks", "POST", "/v2/webhooks", "postWebhook", false, None),
        op("Webhooks", "GET", "/v2/webhooks/{webhook_id}", "getWebhook", false, None),
        op("Webhooks", "PUT", "/v2/webhooks/{webhook_id}", "putWebhook", false, None),
        op("Webhooks", "DELETE", "/v2/webhooks/{webhook_id}", "deleteWebhook", false, None),
        op("Webhooks", "GET", "/v2/teams/{team_id}/webhooks", "getTeamWebhooks", true, None),
        op("Webhooks", "GET", "/v2/webhooks/{webhook_id}/requests", "getWebhookRequests", false, None),
        op("Activity Logs", "GET", "/v1/activity_logs", "getActivityLogs", false, None),
        op("Developer Logs", "POST", "/v1/developer_logs", "getDeveloperLogs", false, Some("readonly")),
        op("Payments", "GET", "/v1/payments", "getPayments", false, None),
        op("AI Usage", "GET", "/v1/ai_usage/daily", "getAiUsageDaily", false, None),
        op("Variables", "GET", "/v1/files/{file_key}/variables/local", "getLocalVariables", false, None),
        op("Variables", "GET", "/v1/files/{file_key}/variables/published", "getPublishedVariables", false, None),
        op("Variables", "POST", "/v1/files/{file_key}/variables", "postVariables", false, None),
        op("Dev Resources", "GET", "/v1/files/{file_key}/dev_resources", "getDevResources", false, None),
        op("Dev Resources", "POST", "/v1/dev_resources", "postDevResources", false, None),
        op("Dev Resources", "PUT", "/v1/dev_resources", "putDevResources", false, None),
        op("Dev Resources", "DELETE", "/v1/files/{file_key}/dev_resources/{dev_resource_id}", "deleteDevResource", false, None),
        op("Library Analytics", "GET", "/v1/analytics/libraries/{file_key}/component/actions", "getLibraryAnalyticsComponentActions", false, None),
        op("Library Analytics", "GET", "/v1/analytics/libraries/{file_key}/component/usages", "getLibraryAnalyticsComponentUsages", false, None),
        op("Library Analytics", "GET", "/v1/analytics/libraries/{file_key}/style/actions", "getLibraryAnalyticsStyleActions", false, None),
        op("Library Analytics", "GET", "/v1/analytics/libraries/{file_key}/style/usages", "getLibraryAnalyticsStyleUsages", false, None),
        op("Library Analytics", "GET", "/v1/analytics/libraries/{file_key}/variable/actions", "getLibraryAnalyticsVariableActions", false, None),
        op("Library Analytics", "GET", "/v1/analytics/libraries/{file_key}/variable/usages", "getLibraryAnalyticsVariableUsages", false, None),
        op("oEmbed", "GET", "/v1/oembed", "getOembed", false, None),
    ]
}

/// Product capabilities exposed only through official Figma MCP surfaces.
pub fn official_mcp_only_capabilities() -> Value {
    json!([
        {"capability": "native_canvas_write",
         "status": "unsupported-by-public-rest-api",
         "reason": "Figma MCP use_figma-style native canvas mutations are not exposed by the public REST OpenAPI."},
        {"capability": "code_to_canvas",
         "status": "unsupported-by-public-rest-api",
         "reason": "generate_figma_design is an official MCP product capability, not a public REST endpoint."},
        {"capability": "code_connect_auto_mapping",
         "status": "unsupported-by-public-rest-api",
         "reason": "Automatic Code Connect mapping discovery is tied to Figma's product integration."},
        {"capability": "make_resources",
         "status": "unsupported-by-public-rest-api",
         "reason": "Make resources are exposed through official MCP product surfaces, not this REST snapshot."},
        {"capability": "figjam_generation",
         "status": "unsupported-by-public-rest-api",
         "reason": "FigJam generation is not present in the public REST OpenAPI snapshot."}
    ])
}

/// Look up an operation by its operationId.
pub fn operation_by_id(operation_id: &str) -> Option<Operation> {
    official_operations()
        .into_iter()
        .find(|o| o.operation_id == operation_id)
}

/// Convert a camelCase operationId to snake_case.
pub fn camel_to_snake(value: &str) -> String {
    // (str/replace #"([a-z0-9])([A-Z])" "$1_$2") then replace "-" and lower-case.
    let mut out = String::with_capacity(value.len() + 4);
    let chars: Vec<char> = value.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                if prev.is_ascii_lowercase() || prev.is_ascii_digit() {
                    out.push('_');
                }
            }
            out.push(c);
        } else {
            out.push(c);
        }
    }
    out.replace('-', "_").to_lowercase()
}

/// MCP tool name for an operationId (`figma_<snake>`).
pub fn mcp_tool_name(operation_id: &str) -> String {
    format!("figma_{}", camel_to_snake(operation_id))
}

/// Reverse-lookup an operation from its `figma_*` MCP tool name.
pub fn operation_for_tool_name(tool_name: &str) -> Option<Operation> {
    let needle = tool_name.strip_prefix("figma_")?;
    official_operations()
        .into_iter()
        .find(|o| camel_to_snake(o.operation_id) == needle)
}

/// Aggregate summary counts, grouped by tag (sorted).
pub fn operation_summary() -> Value {
    let ops = official_operations();
    let covered = ops.iter().filter(|o| o.status == "covered").count();
    let write = ops.iter().filter(|o| o.is_write()).count();
    let deprecated = ops.iter().filter(|o| o.deprecated).count();

    // group-by tag, sorted map.
    let mut tags: std::collections::BTreeMap<&str, (usize, usize, usize)> =
        std::collections::BTreeMap::new();
    for o in &ops {
        let e = tags.entry(o.tag).or_insert((0, 0, 0));
        e.0 += 1;
        if o.is_write() {
            e.1 += 1;
        }
        if o.deprecated {
            e.2 += 1;
        }
    }
    let mut by_tag = serde_json::Map::new();
    for (tag, (total, w, dep)) in tags {
        by_tag.insert(
            tag.to_string(),
            json!({"total": total, "write": w, "deprecated": dep}),
        );
    }

    json!({
        "openapi_version": OPENAPI_VERSION,
        "operation_count": ops.len(),
        "covered_count": covered,
        "write_count": write,
        "deprecated_count": deprecated,
        "by_tag": Value::Object(by_tag),
    })
}

/// Full machine-readable coverage report.
pub fn coverage_report() -> Value {
    let ops = official_operations();
    let operations: Vec<Value> = ops
        .iter()
        .map(|o| {
            // Key order is fixed for stable, deterministic JSON output.
            json!({
                "path": o.path,
                "method": o.method,
                "cli_command": format!("fighorse figma api {} --params '{{...}}'", o.operation_id),
                "policy": o.policy,
                "operation_id": o.operation_id,
                "status": o.status,
                "deprecated": o.deprecated,
                "mcp_tool": mcp_tool_name(o.operation_id),
                "tag": o.tag,
            })
        })
        .collect();

    json!({
        "kind": "fighorse.api-coverage.v1",
        "source": {
            "name": "Figma REST OpenAPI",
            "version": OPENAPI_VERSION,
            "url": "https://github.com/figma/rest-api-spec/blob/main/openapi/openapi.yaml"
        },
        "summary": operation_summary(),
        "operations": operations,
        "official_mcp_only": official_mcp_only_capabilities(),
        "ai_guidance": {
            "use": "Use this report to verify REST parity before relying on low-level Figma tools.",
            "next_step": "If any operation is not covered, update fighorse.api operations, MCP tools, CLI dispatch, discovery, and tests together."
        }
    })
}

/// Markdown rendering of the coverage report.
pub fn coverage_report_markdown(report: &Value) -> String {
    let source_version = report["source"]["version"].as_str().unwrap_or("");
    let op_count = report["summary"]["operation_count"].as_u64().unwrap_or(0);
    let covered = report["summary"]["covered_count"].as_u64().unwrap_or(0);
    let write = report["summary"]["write_count"].as_u64().unwrap_or(0);

    let mut out = String::new();
    out.push_str("# fighorse Figma REST API Coverage\n\n");
    out.push_str(&format!("- OpenAPI version: `{source_version}`\n"));
    out.push_str(&format!("- Operations: `{op_count}`\n"));
    out.push_str(&format!("- Covered: `{covered}`\n"));
    out.push_str(&format!("- Write operations: `{write}`\n\n"));
    out.push_str("## Operations\n\n");

    let ops: Vec<String> = report["operations"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|o| {
                    let method = o["method"].as_str().unwrap_or("");
                    let path = o["path"].as_str().unwrap_or("");
                    let oid = o["operation_id"].as_str().unwrap_or("");
                    let tag = o["tag"].as_str().unwrap_or("");
                    let status = o["status"].as_str().unwrap_or("");
                    let mcp = o["mcp_tool"].as_str().unwrap_or("");
                    let deprecated = o["deprecated"].as_bool().unwrap_or(false);
                    let dep = if deprecated { " deprecated=true" } else { "" };
                    format!(
                        "- `{method} {path}` `{oid}` [{tag}] status=`{status}` mcp=`{mcp}`{dep}"
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    out.push_str(&ops.join("\n"));

    out.push_str("\n\n## Official MCP Product-Only Capabilities\n\n");
    let caps: Vec<String> = report["official_mcp_only"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|c| {
                    let cap = c["capability"].as_str().unwrap_or("");
                    let status = c["status"].as_str().unwrap_or("");
                    let reason = c["reason"].as_str().unwrap_or("");
                    format!("- `{cap}` status=`{status}`: {reason}")
                })
                .collect()
        })
        .unwrap_or_default();
    out.push_str(&caps.join("\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_complete() {
        assert_eq!(OPENAPI_VERSION, "0.41.0");
        let ops = official_operations();
        assert_eq!(ops.len(), 50);
        let distinct: std::collections::HashSet<_> =
            ops.iter().map(|o| o.operation_id).collect();
        assert_eq!(distinct.len(), 50);

        let report = coverage_report();
        assert_eq!(report["summary"]["covered_count"], 50);
        assert_eq!(report["summary"]["write_count"], 11);
    }

    #[test]
    fn contains_known_operations() {
        let ids: std::collections::HashSet<_> = official_operations()
            .iter()
            .map(|o| o.operation_id.to_string())
            .collect();
        for id in [
            "getDeveloperLogs",
            "postVariables",
            "postDevResources",
            "putDevResources",
            "putWebhook",
            "getWebhookRequests",
            "getLibraryAnalyticsStyleActions",
        ] {
            assert!(ids.contains(id), "missing {id}");
        }
    }

    #[test]
    fn derives_tool_names() {
        assert_eq!(mcp_tool_name("getFile"), "figma_get_file");
        assert_eq!(
            operation_for_tool_name("figma_put_webhook").unwrap().operation_id,
            "putWebhook"
        );
    }
}
