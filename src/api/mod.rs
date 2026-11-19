//! Figma REST API endpoint wrappers.
//!
//! Each submodule mirrors the corresponding `fighorse.api.*` namespace. Query
//! parameters accept `Option<&str>`; `None` maps to a JSON `null` which the URL
//! builder omits.

pub mod code_connect;
pub mod coverage;
pub mod operations;

use serde_json::Value;

/// Convert an optional string param into a JSON value (`None` -> `Null`).
pub(crate) fn opt(v: Option<&str>) -> Value {
    match v {
        Some(s) => Value::String(s.to_string()),
        None => Value::Null,
    }
}

pub mod files {
    //! Figma Files API.
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    #[derive(Debug, Clone, Copy, Default)]
    pub struct GetFileParams<'a> {
        pub version: Option<&'a str>,
        pub ids: Option<&'a str>,
        pub depth: Option<&'a str>,
        pub geometry: Option<&'a str>,
        pub plugin_data: Option<&'a str>,
        pub branch_data: Option<&'a str>,
    }

    pub async fn get_file(token: &str, file_key: &str, params: GetFileParams<'_>) -> Result<Value> {
        let path = format!("/v1/files/{}", http::path_segment(file_key));
        http::get(
            &path,
            Some(token),
            &[
                ("version", opt(params.version)),
                ("ids", opt(params.ids)),
                ("depth", opt(params.depth)),
                ("geometry", opt(params.geometry)),
                ("plugin_data", opt(params.plugin_data)),
                ("branch_data", opt(params.branch_data)),
            ],
        )
        .await
    }

    pub async fn get_file_nodes(
        token: &str,
        file_key: &str,
        ids: &str,
        version: Option<&str>,
        depth: Option<&str>,
        geometry: Option<&str>,
        plugin_data: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v1/files/{}/nodes", http::path_segment(file_key));
        http::get(
            &path,
            Some(token),
            &[
                ("ids", Value::String(ids.to_string())),
                ("version", opt(version)),
                ("depth", opt(depth)),
                ("geometry", opt(geometry)),
                ("plugin_data", opt(plugin_data)),
            ],
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn get_images(
        token: &str,
        file_key: &str,
        ids: &str,
        version: Option<&str>,
        scale: Option<&str>,
        format: Option<&str>,
        svg_outline_text: Option<&str>,
        svg_include_id: Option<&str>,
        svg_include_node_id: Option<&str>,
        svg_simplify_stroke: Option<&str>,
        contents_only: Option<&str>,
        use_absolute_bounds: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v1/images/{}", http::path_segment(file_key));
        // Defaults: format "png", scale 2.
        let scale_v = match scale {
            Some(s) => Value::String(s.to_string()),
            None => Value::String("2".to_string()),
        };
        let format_v = match format {
            Some(s) => Value::String(s.to_string()),
            None => Value::String("png".to_string()),
        };
        http::get(
            &path,
            Some(token),
            &[
                ("ids", Value::String(ids.to_string())),
                ("version", opt(version)),
                ("scale", scale_v),
                ("format", format_v),
                ("svg_outline_text", opt(svg_outline_text)),
                ("svg_include_id", opt(svg_include_id)),
                ("svg_include_node_id", opt(svg_include_node_id)),
                ("svg_simplify_stroke", opt(svg_simplify_stroke)),
                ("contents_only", opt(contents_only)),
                ("use_absolute_bounds", opt(use_absolute_bounds)),
            ],
        )
        .await
    }

    pub async fn get_image_fills(token: &str, file_key: &str) -> Result<Value> {
        let path = format!("/v1/files/{}/images", http::path_segment(file_key));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn get_file_meta(token: &str, file_key: &str) -> Result<Value> {
        let path = format!("/v1/files/{}/meta", http::path_segment(file_key));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn get_file_versions(
        token: &str,
        file_key: &str,
        page_size: Option<&str>,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v1/files/{}/versions", http::path_segment(file_key));
        http::get(
            &path,
            Some(token),
            &[
                ("page_size", opt(page_size)),
                ("before", opt(before)),
                ("after", opt(after)),
            ],
        )
        .await
    }

    /// POST /v1/files/{file_key}/duplicate - duplicate a main file or branch into a folder.
    pub async fn duplicate_file(token: &str, file_key: &str, folder_id: &str) -> Result<Value> {
        let path = format!("/v1/files/{}/duplicate", http::path_segment(file_key));
        let body = serde_json::json!({ "folder_id": folder_id });
        http::post(&path, Some(token), &[], Some(&body)).await
    }
}

pub mod projects {
    //! Legacy Figma Projects API (deprecated by Figma in favor of `folders`).
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    pub async fn get_team_projects(token: &str, team_id: &str) -> Result<Value> {
        let path = format!("/v1/teams/{}/projects", http::path_segment(team_id));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn get_project_files(
        token: &str,
        project_id: &str,
        branch_data: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v1/projects/{}/files", http::path_segment(project_id));
        http::get(&path, Some(token), &[("branch_data", opt(branch_data))]).await
    }

    /// GET /v1/projects/{project_id}/meta - project metadata.
    pub async fn get_project_meta(token: &str, project_id: &str) -> Result<Value> {
        let path = format!("/v1/projects/{}/meta", http::path_segment(project_id));
        http::get(&path, Some(token), &[]).await
    }
}

pub mod folders {
    //! Figma v2 Folders API (replaces deprecated Projects endpoints and adds nested subfolder traversal).
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::{Value, json};

    /// GET /v2/teams/{team_id}/folders - top-level folders in a team.
    pub async fn get_team_folders(token: &str, team_id: &str) -> Result<Value> {
        let path = format!("/v2/teams/{}/folders", http::path_segment(team_id));
        http::get(&path, Some(token), &[]).await
    }

    /// POST /v1/teams/{team_id}/folders - create a top-level folder in a team.
    pub async fn post_team_folder(
        token: &str,
        team_id: &str,
        name: &str,
        sharing_audience_control: &str,
    ) -> Result<Value> {
        let path = format!("/v1/teams/{}/folders", http::path_segment(team_id));
        let body = json!({
            "name": name,
            "sharing_audience_control": sharing_audience_control,
        });
        http::post(&path, Some(token), &[], Some(&body)).await
    }

    /// POST /v1/teams/{team_id}/folders accepting a raw JSON body.
    pub async fn post_team_folder_body(token: &str, team_id: &str, body: &Value) -> Result<Value> {
        let path = format!("/v1/teams/{}/folders", http::path_segment(team_id));
        http::post(&path, Some(token), &[], Some(body)).await
    }

    /// GET /v2/folders/{folder_id}/folders - direct subfolders within a folder.
    pub async fn get_folder_folders(token: &str, folder_id: &str) -> Result<Value> {
        let path = format!("/v2/folders/{}/folders", http::path_segment(folder_id));
        http::get(&path, Some(token), &[]).await
    }

    /// GET /v2/folders/{folder_id}/files - files directly within a folder.
    pub async fn get_folder_files(
        token: &str,
        folder_id: &str,
        branch_data: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v2/folders/{}/files", http::path_segment(folder_id));
        http::get(&path, Some(token), &[("branch_data", opt(branch_data))]).await
    }

    /// GET /v2/folders/{folder_id}/meta - folder metadata.
    pub async fn get_folder_meta(token: &str, folder_id: &str) -> Result<Value> {
        let path = format!("/v2/folders/{}/meta", http::path_segment(folder_id));
        http::get(&path, Some(token), &[]).await
    }
}

pub mod teams {
    //! Figma Teams management API.
    use crate::error::Result;
    use crate::http;
    use serde_json::{Value, json};

    /// GET /v1/teams/{team_id} - retrieve team metadata.
    pub async fn get_team(token: &str, team_id: &str) -> Result<Value> {
        let path = format!("/v1/teams/{}", http::path_segment(team_id));
        http::get(&path, Some(token), &[]).await
    }

    /// POST /v1/teams - create a team in an Enterprise plan.
    pub async fn post_team(token: &str, body: &Value) -> Result<Value> {
        http::post("/v1/teams", Some(token), &[], Some(body)).await
    }

    /// POST /v1/teams/{team_id}/invites - invite users to a team.
    pub async fn post_team_invites(token: &str, team_id: &str, body: &Value) -> Result<Value> {
        let path = format!("/v1/teams/{}/invites", http::path_segment(team_id));
        http::post(&path, Some(token), &[], Some(body)).await
    }

    /// Build a `CreateTeamRequest` JSON payload.
    pub fn build_create_team_body(
        name: &str,
        sharing_audience_control: &str,
        description: Option<&str>,
        org_discoverability: Option<&str>,
        workspace_id: Option<&str>,
    ) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("name".into(), Value::String(name.to_string()));
        map.insert(
            "sharing_audience_control".into(),
            Value::String(sharing_audience_control.to_string()),
        );
        if let Some(desc) = description {
            map.insert("description".into(), Value::String(desc.to_string()));
        }
        if let Some(disc) = org_discoverability {
            map.insert(
                "org_discoverability".into(),
                Value::String(disc.to_string()),
            );
        }
        if let Some(ws) = workspace_id {
            map.insert("workspace_id".into(), Value::String(ws.to_string()));
        }
        Value::Object(map)
    }

    /// Build an `InviteTeamMembersRequest` JSON payload.
    pub fn build_invite_body(emails: &[String], role: &str) -> Value {
        json!({
            "emails": emails,
            "role": role,
        })
    }
}

pub mod users {
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    pub async fn get_me(token: &str) -> Result<Value> {
        http::get("/v1/me", Some(token), &[]).await
    }
}

pub mod components {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    pub async fn get_team_components(
        token: &str,
        team_id: &str,
        page_size: Option<&str>,
        after: Option<&str>,
        before: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v1/teams/{}/components", http::path_segment(team_id));
        http::get(
            &path,
            Some(token),
            &[
                ("page_size", opt(page_size)),
                ("after", opt(after)),
                ("before", opt(before)),
            ],
        )
        .await
    }

    pub async fn get_file_components(token: &str, file_key: &str) -> Result<Value> {
        let path = format!("/v1/files/{}/components", http::path_segment(file_key));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn get_component(token: &str, key: &str) -> Result<Value> {
        let path = format!("/v1/components/{}", http::path_segment(key));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn get_team_component_sets(
        token: &str,
        team_id: &str,
        page_size: Option<&str>,
        after: Option<&str>,
        before: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v1/teams/{}/component_sets", http::path_segment(team_id));
        http::get(
            &path,
            Some(token),
            &[
                ("page_size", opt(page_size)),
                ("after", opt(after)),
                ("before", opt(before)),
            ],
        )
        .await
    }

    pub async fn get_file_component_sets(token: &str, file_key: &str) -> Result<Value> {
        let path = format!("/v1/files/{}/component_sets", http::path_segment(file_key));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn get_component_set(token: &str, key: &str) -> Result<Value> {
        let path = format!("/v1/component_sets/{}", http::path_segment(key));
        http::get(&path, Some(token), &[]).await
    }
}

pub mod styles {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    pub async fn get_team_styles(
        token: &str,
        team_id: &str,
        page_size: Option<&str>,
        after: Option<&str>,
        before: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v1/teams/{}/styles", http::path_segment(team_id));
        http::get(
            &path,
            Some(token),
            &[
                ("page_size", opt(page_size)),
                ("after", opt(after)),
                ("before", opt(before)),
            ],
        )
        .await
    }

    pub async fn get_file_styles(token: &str, file_key: &str) -> Result<Value> {
        let path = format!("/v1/files/{}/styles", http::path_segment(file_key));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn get_style(token: &str, key: &str) -> Result<Value> {
        let path = format!("/v1/styles/{}", http::path_segment(key));
        http::get(&path, Some(token), &[]).await
    }
}

pub mod comments {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::{Value, json};

    pub async fn get_comments(token: &str, file_key: &str, as_md: Option<&str>) -> Result<Value> {
        let path = format!("/v1/files/{}/comments", http::path_segment(file_key));
        http::get(&path, Some(token), &[("as_md", opt(as_md))]).await
    }

    pub async fn post_comment(
        token: &str,
        file_key: &str,
        message: Option<&str>,
        comment_id: Option<&str>,
        client_meta: Option<&Value>,
    ) -> Result<Value> {
        let path = format!("/v1/files/{}/comments", http::path_segment(file_key));
        let body = json!({
            "message": opt(message),
            "comment_id": opt(comment_id),
            "client_meta": client_meta.cloned().unwrap_or(Value::Null),
        });
        http::post(&path, Some(token), &[], Some(&body)).await
    }

    pub async fn delete_comment(token: &str, file_key: &str, comment_id: &str) -> Result<Value> {
        let path = format!(
            "/v1/files/{}/comments/{}",
            http::path_segment(file_key),
            http::path_segment(comment_id)
        );
        http::delete(&path, Some(token), &[]).await
    }

    pub async fn get_comment_reactions(
        token: &str,
        file_key: &str,
        comment_id: &str,
        cursor: Option<&str>,
    ) -> Result<Value> {
        let path = format!(
            "/v1/files/{}/comments/{}/reactions",
            http::path_segment(file_key),
            http::path_segment(comment_id)
        );
        http::get(&path, Some(token), &[("cursor", opt(cursor))]).await
    }

    pub async fn post_comment_reaction(
        token: &str,
        file_key: &str,
        comment_id: &str,
        emoji: Option<&str>,
    ) -> Result<Value> {
        let path = format!(
            "/v1/files/{}/comments/{}/reactions",
            http::path_segment(file_key),
            http::path_segment(comment_id)
        );
        let body = json!({ "emoji": opt(emoji) });
        http::post(&path, Some(token), &[], Some(&body)).await
    }

    pub async fn delete_comment_reaction(
        token: &str,
        file_key: &str,
        comment_id: &str,
        emoji: Option<&str>,
    ) -> Result<Value> {
        let path = format!(
            "/v1/files/{}/comments/{}/reactions",
            http::path_segment(file_key),
            http::path_segment(comment_id)
        );
        http::delete(&path, Some(token), &[("emoji", opt(emoji))]).await
    }

    /// Optional client-side filters over `/v1/files/{file_key}/comments`.
    #[derive(Debug, Clone, Default)]
    pub struct CommentFilterOpts<'a> {
        pub node_id: Option<&'a str>,
        pub author: Option<&'a str>,
        pub query: Option<&'a str>,
        pub unresolved_only: bool,
        pub threads_only: bool,
    }

    impl CommentFilterOpts<'_> {
        pub fn is_active(&self) -> bool {
            self.node_id.is_some_and(|v| !v.trim().is_empty())
                || self.author.is_some_and(|v| !v.trim().is_empty())
                || self.query.is_some_and(|v| !v.trim().is_empty())
                || self.unresolved_only
                || self.threads_only
        }
    }

    /// Filter a raw `get_comments` JSON payload while preserving complete matching threads.
    pub fn filter_comments(raw: &Value, opts: &CommentFilterOpts<'_>) -> Value {
        if !opts.is_active() {
            return raw.clone();
        }
        let comments = raw
            .get("comments")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let target_node = opts
            .node_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(crate::url::normalize_node_id);
        let target_author = opts
            .author
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());
        let target_query = opts
            .query
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());

        let is_root = |c: &Value| {
            c.get("parent_id")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
        };

        // Determine matching thread root IDs so replies stay attached to matching threads.
        let mut matched_roots = std::collections::HashSet::new();
        for c in &comments {
            let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let root_id = c
                .get("parent_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(id);

            let node_ok = match &target_node {
                Some(nid) => c
                    .get("client_meta")
                    .and_then(|m| m.get("node_id"))
                    .and_then(|v| v.as_str())
                    .map(|v| crate::url::normalize_node_id(v) == *nid)
                    .unwrap_or(false),
                None => true,
            };
            let author_ok = match &target_author {
                Some(author) => c
                    .get("user")
                    .and_then(|u| u.get("handle").or_else(|| u.get("id")))
                    .and_then(|v| v.as_str())
                    .map(|h| h.to_ascii_lowercase().contains(author))
                    .unwrap_or(false),
                None => true,
            };
            let query_ok = match &target_query {
                Some(q) => c
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(|m| m.to_ascii_lowercase().contains(q))
                    .unwrap_or(false),
                None => true,
            };

            if node_ok && author_ok && query_ok && !root_id.is_empty() {
                matched_roots.insert(root_id.to_string());
            }
        }

        // Apply unresolved filter against thread roots.
        let mut unresolved_roots = std::collections::HashSet::new();
        if opts.unresolved_only {
            for c in &comments {
                if is_root(c) {
                    let resolved = c
                        .get("resolved_at")
                        .and_then(|v| v.as_str())
                        .is_some_and(|s| !s.is_empty());
                    if !resolved {
                        if let Some(id) = c.get("id").and_then(|v| v.as_str()) {
                            unresolved_roots.insert(id.to_string());
                        }
                    }
                }
            }
        }

        let filtered: Vec<Value> = comments
            .iter()
            .filter(|c| {
                let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let root_id = c
                    .get("parent_id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(id);
                if (target_node.is_some() || target_author.is_some() || target_query.is_some())
                    && !matched_roots.contains(root_id)
                {
                    return false;
                }
                if opts.unresolved_only && !unresolved_roots.contains(root_id) {
                    return false;
                }
                if opts.threads_only && !is_root(c) {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        json!({
            "comments": filtered,
            "filter_summary": {
                "total_comments": comments.len(),
                "matched_comments": filtered.len(),
                "node_id": target_node,
                "author": opts.author,
                "query": opts.query,
                "unresolved_only": opts.unresolved_only,
                "threads_only": opts.threads_only,
            }
        })
    }
}

pub mod webhooks {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    pub async fn get_webhooks(
        token: &str,
        context: Option<&str>,
        plan: Option<&str>,
        after: Option<&str>,
    ) -> Result<Value> {
        http::get(
            "/v2/webhooks",
            Some(token),
            &[
                ("context", opt(context)),
                ("plan", opt(plan)),
                ("after", opt(after)),
            ],
        )
        .await
    }

    pub async fn create_webhook(token: &str, body: &Value) -> Result<Value> {
        http::post("/v2/webhooks", Some(token), &[], Some(body)).await
    }

    pub async fn get_webhook(token: &str, webhook_id: &str) -> Result<Value> {
        let path = format!("/v2/webhooks/{}", http::path_segment(webhook_id));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn update_webhook(token: &str, webhook_id: &str, body: &Value) -> Result<Value> {
        let path = format!("/v2/webhooks/{}", http::path_segment(webhook_id));
        http::put(&path, Some(token), &[], Some(body)).await
    }

    pub async fn delete_webhook(token: &str, webhook_id: &str) -> Result<Value> {
        let path = format!("/v2/webhooks/{}", http::path_segment(webhook_id));
        http::delete(&path, Some(token), &[]).await
    }

    pub async fn get_team_webhooks(token: &str, team_id: &str) -> Result<Value> {
        let path = format!("/v2/teams/{}/webhooks", http::path_segment(team_id));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn get_webhook_requests(
        token: &str,
        webhook_id: &str,
        cursor: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v2/webhooks/{}/requests", http::path_segment(webhook_id));
        http::get(&path, Some(token), &[("cursor", opt(cursor))]).await
    }
}

pub mod variables {
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    pub async fn get_local_variables(token: &str, file_key: &str) -> Result<Value> {
        let path = format!("/v1/files/{}/variables/local", http::path_segment(file_key));
        http::get(&path, Some(token), &[]).await
    }

    pub async fn get_published_variables(token: &str, file_key: &str) -> Result<Value> {
        let path = format!(
            "/v1/files/{}/variables/published",
            http::path_segment(file_key)
        );
        http::get(&path, Some(token), &[]).await
    }

    pub async fn post_variables(token: &str, file_key: &str, changes: &Value) -> Result<Value> {
        let path = format!("/v1/files/{}/variables", http::path_segment(file_key));
        http::post(&path, Some(token), &[], Some(changes)).await
    }
}

pub mod dev_resources {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::{Value, json};

    pub async fn get_dev_resources(
        token: &str,
        file_key: &str,
        node_ids: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v1/files/{}/dev_resources", http::path_segment(file_key));
        http::get(&path, Some(token), &[("node_ids", opt(node_ids))]).await
    }

    pub async fn post_dev_resources(token: &str, dev_resources: &Value) -> Result<Value> {
        let body = json!({ "dev_resources": dev_resources });
        http::post("/v1/dev_resources", Some(token), &[], Some(&body)).await
    }

    pub async fn put_dev_resources(token: &str, dev_resources: &Value) -> Result<Value> {
        let body = json!({ "dev_resources": dev_resources });
        http::put("/v1/dev_resources", Some(token), &[], Some(&body)).await
    }

    pub async fn delete_dev_resource(
        token: &str,
        file_key: &str,
        dev_resource_id: &str,
    ) -> Result<Value> {
        let path = format!(
            "/v1/files/{}/dev_resources/{}",
            http::path_segment(file_key),
            http::path_segment(dev_resource_id)
        );
        http::delete(&path, Some(token), &[]).await
    }
}

pub mod analytics {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    async fn usage_actions(
        token: &str,
        file_key: &str,
        suffix: &str,
        cursor: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        order_direction: Option<&str>,
    ) -> Result<Value> {
        let path = format!(
            "/v1/analytics/libraries/{}/{}",
            http::path_segment(file_key),
            suffix
        );
        http::get(
            &path,
            Some(token),
            &[
                ("cursor", opt(cursor)),
                ("start_date", opt(start_date)),
                ("end_date", opt(end_date)),
                ("order_direction", opt(order_direction)),
            ],
        )
        .await
    }

    pub async fn component_usages(
        token: &str,
        file_key: &str,
        cursor: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        order_direction: Option<&str>,
    ) -> Result<Value> {
        usage_actions(
            token,
            file_key,
            "component/usages",
            cursor,
            start_date,
            end_date,
            order_direction,
        )
        .await
    }

    pub async fn component_actions(
        token: &str,
        file_key: &str,
        cursor: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        order_direction: Option<&str>,
    ) -> Result<Value> {
        usage_actions(
            token,
            file_key,
            "component/actions",
            cursor,
            start_date,
            end_date,
            order_direction,
        )
        .await
    }

    pub async fn style_usages(
        token: &str,
        file_key: &str,
        cursor: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        order_direction: Option<&str>,
    ) -> Result<Value> {
        usage_actions(
            token,
            file_key,
            "style/usages",
            cursor,
            start_date,
            end_date,
            order_direction,
        )
        .await
    }

    pub async fn style_actions(
        token: &str,
        file_key: &str,
        cursor: Option<&str>,
        group_by: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> Result<Value> {
        let path = format!(
            "/v1/analytics/libraries/{}/style/actions",
            http::path_segment(file_key)
        );
        http::get(
            &path,
            Some(token),
            &[
                ("cursor", opt(cursor)),
                ("group_by", opt(group_by)),
                ("start_date", opt(start_date)),
                ("end_date", opt(end_date)),
            ],
        )
        .await
    }

    pub async fn variable_usages(
        token: &str,
        file_key: &str,
        cursor: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        order_direction: Option<&str>,
    ) -> Result<Value> {
        usage_actions(
            token,
            file_key,
            "variable/usages",
            cursor,
            start_date,
            end_date,
            order_direction,
        )
        .await
    }

    pub async fn variable_actions(
        token: &str,
        file_key: &str,
        cursor: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        order_direction: Option<&str>,
    ) -> Result<Value> {
        usage_actions(
            token,
            file_key,
            "variable/actions",
            cursor,
            start_date,
            end_date,
            order_direction,
        )
        .await
    }
}

pub mod activity_logs {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    pub async fn get_activity_logs(
        token: &str,
        start_time: Option<&str>,
        end_time: Option<&str>,
        events: Option<&str>,
        limit: Option<&str>,
        order: Option<&str>,
    ) -> Result<Value> {
        http::get(
            "/v1/activity_logs",
            Some(token),
            &[
                ("start_time", opt(start_time)),
                ("end_time", opt(end_time)),
                ("events", opt(events)),
                ("limit", opt(limit)),
                ("order", opt(order)),
            ],
        )
        .await
    }
}

pub mod developer_logs {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::{Value, json};

    #[allow(clippy::too_many_arguments)]
    pub async fn get_developer_logs(
        token: &str,
        token_type: Option<&str>,
        token_value: Option<&str>,
        token_name: Option<&str>,
        user_email: Option<&str>,
        ip_address: Option<&str>,
        event_source: Option<&str>,
        date_range: Option<&str>,
        limit: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<Value> {
        let body = json!({
            "token_type": opt(token_type),
            "token": opt(token_value),
            "token_name": opt(token_name),
            "user_email": opt(user_email),
            "ip_address": opt(ip_address),
            "event_source": opt(event_source),
            "date_range": opt(date_range),
            "limit": opt(limit),
            "cursor": opt(cursor),
        });
        http::post("/v1/developer_logs", Some(token), &[], Some(&body)).await
    }
}

pub mod ai_usage {
    //! Figma AI Usage API - daily AI usage analytics for an organization.
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    /// GET /v1/ai_usage/daily - daily AI usage rows for an organization.
    #[allow(clippy::too_many_arguments)]
    pub async fn get_ai_usage_daily(
        token: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        user_email: Option<&str>,
        limit: Option<&str>,
        cursor: Option<&str>,
    ) -> Result<Value> {
        http::get(
            "/v1/ai_usage/daily",
            Some(token),
            &[
                ("start_date", opt(start_date)),
                ("end_date", opt(end_date)),
                ("user_email", opt(user_email)),
                ("limit", opt(limit)),
                ("cursor", opt(cursor)),
            ],
        )
        .await
    }
}

pub mod payments {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    pub async fn get_payments(
        token: &str,
        plugin_payment_token: Option<&str>,
        user_id: Option<&str>,
        community_file_id: Option<&str>,
        plugin_id: Option<&str>,
        widget_id: Option<&str>,
    ) -> Result<Value> {
        http::get(
            "/v1/payments",
            Some(token),
            &[
                ("plugin_payment_token", opt(plugin_payment_token)),
                ("user_id", opt(user_id)),
                ("community_file_id", opt(community_file_id)),
                ("plugin_id", opt(plugin_id)),
                ("widget_id", opt(widget_id)),
            ],
        )
        .await
    }
}

pub mod oembed {
    use super::opt;
    use crate::error::Result;
    use crate::http;
    use serde_json::Value;

    pub async fn get_oembed(
        url: Option<&str>,
        max_width: Option<&str>,
        max_height: Option<&str>,
    ) -> Result<Value> {
        http::get(
            "/v1/oembed",
            None,
            &[
                ("url", opt(url)),
                ("max_width", opt(max_width)),
                ("max_height", opt(max_height)),
            ],
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::comments::{CommentFilterOpts, filter_comments};
    use serde_json::json;

    #[test]
    fn filter_comments_preserves_thread_replies_unless_threads_only() {
        let raw = json!({
            "comments": [
                {"id": "1", "message": "Header spacing", "client_meta": {"node_id": "5:10"}, "user": {"handle": "Alice"}, "resolved_at": null},
                {"id": "2", "parent_id": "1", "message": "Updated to 16px", "user": {"handle": "Bob"}, "resolved_at": null},
                {"id": "3", "message": "Resolved thread", "client_meta": {"node_id": "5:10"}, "user": {"handle": "Alice"}, "resolved_at": "2026-11-01T00:00:00Z"}
            ]
        });

        let full_thread = filter_comments(
            &raw,
            &CommentFilterOpts {
                node_id: Some("5-10"),
                unresolved_only: true,
                ..Default::default()
            },
        );
        assert_eq!(full_thread["comments"].as_array().unwrap().len(), 2);

        let roots_only = filter_comments(
            &raw,
            &CommentFilterOpts {
                author: Some("bob"),
                threads_only: true,
                ..Default::default()
            },
        );
        assert_eq!(roots_only["comments"].as_array().unwrap().len(), 1);
        assert_eq!(roots_only["comments"][0]["id"], "1");
    }
}
