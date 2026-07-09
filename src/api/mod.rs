//! Figma REST API endpoint wrappers.
//!
//! Each submodule mirrors the corresponding `fighorse.api.*` namespace. Query
//! parameters accept `Option<&str>`; `None` maps to a JSON `null` which the URL
//! builder omits.

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

    pub async fn get_file(
        token: &str,
        file_key: &str,
        version: Option<&str>,
        ids: Option<&str>,
        depth: Option<&str>,
        geometry: Option<&str>,
        plugin_data: Option<&str>,
        branch_data: Option<&str>,
    ) -> Result<Value> {
        let path = format!("/v1/files/{}", http::path_segment(file_key));
        http::get(
            &path,
            Some(token),
            &[
                ("version", opt(version)),
                ("ids", opt(ids)),
                ("depth", opt(depth)),
                ("geometry", opt(geometry)),
                ("plugin_data", opt(plugin_data)),
                ("branch_data", opt(branch_data)),
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
}

pub mod projects {
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
    use serde_json::{json, Value};

    pub async fn get_comments(
        token: &str,
        file_key: &str,
        as_md: Option<&str>,
    ) -> Result<Value> {
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
    use serde_json::{json, Value};

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
        usage_actions(token, file_key, "component/usages", cursor, start_date, end_date, order_direction).await
    }

    pub async fn component_actions(
        token: &str,
        file_key: &str,
        cursor: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        order_direction: Option<&str>,
    ) -> Result<Value> {
        usage_actions(token, file_key, "component/actions", cursor, start_date, end_date, order_direction).await
    }

    pub async fn style_usages(
        token: &str,
        file_key: &str,
        cursor: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        order_direction: Option<&str>,
    ) -> Result<Value> {
        usage_actions(token, file_key, "style/usages", cursor, start_date, end_date, order_direction).await
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
        usage_actions(token, file_key, "variable/usages", cursor, start_date, end_date, order_direction).await
    }

    pub async fn variable_actions(
        token: &str,
        file_key: &str,
        cursor: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        order_direction: Option<&str>,
    ) -> Result<Value> {
        usage_actions(token, file_key, "variable/actions", cursor, start_date, end_date, order_direction).await
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
    use serde_json::{json, Value};

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
