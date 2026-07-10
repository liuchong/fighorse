//! Operation-id dispatcher for the public Figma REST API.
//!
//! Operation-id dispatcher: maps an operationId + params/body to the
//! matching endpoint wrapper.

use super::coverage;
use super::{
    activity_logs, ai_usage, analytics, comments, components, dev_resources, developer_logs,
    files, oembed, payments, projects, styles, users, variables, webhooks,
};
use crate::error::{Error, Result};
use serde_json::Value;

/// Look up a covered operation by id.
pub fn operation(operation_id: &str) -> Option<coverage::Operation> {
    coverage::operation_by_id(operation_id)
}

/// True when the operation is a write operation.
pub fn write_operation(operation_id: &str) -> bool {
    operation(operation_id).map(|o| o.is_write()).unwrap_or(false)
}

fn snake_to_kebab(k: &str) -> String {
    k.replace('_', "-")
}

/// Owned param that also stringifies numbers/bools (for query values).
fn param_owned(params: &Value, key: &str) -> Option<String> {
    let obj = params.as_object()?;
    let raw = obj.get(key).or_else(|| obj.get(&snake_to_kebab(key)))?;
    match raw {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Body field lookup trying snake then kebab keys.
fn body_field<'a>(body: &'a Value, key: &str) -> Option<&'a Value> {
    let obj = body.as_object()?;
    obj.get(key).or_else(|| obj.get(&snake_to_kebab(key)))
}

fn body_str(body: &Value, key: &str) -> Option<String> {
    body_field(body, key).and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// Extract the `dev_resources` array from a body (defaults to []).
fn dev_resources_body(body: &Value) -> Value {
    body_field(body, "dev_resources")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]))
}

/// Call a covered Figma REST operation by operationId.
pub async fn call_operation(
    token: &str,
    operation_id: &str,
    params: &Value,
    body: &Value,
) -> Result<Value> {
    // Owned strings kept alive for the duration of the call.
    macro_rules! p {
        ($k:expr) => {
            param_owned(params, $k)
        };
    }

    match operation_id {
        "getFile" => {
            files::get_file(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("version").as_deref(),
                p!("ids").as_deref(),
                p!("depth").as_deref(),
                p!("geometry").as_deref(),
                p!("plugin_data").as_deref(),
                p!("branch_data").as_deref(),
            )
            .await
        }
        "getFileNodes" => {
            files::get_file_nodes(
                token,
                &p!("file_key").unwrap_or_default(),
                &p!("ids").unwrap_or_default(),
                p!("version").as_deref(),
                p!("depth").as_deref(),
                p!("geometry").as_deref(),
                p!("plugin_data").as_deref(),
            )
            .await
        }
        "getImages" => {
            files::get_images(
                token,
                &p!("file_key").unwrap_or_default(),
                &p!("ids").unwrap_or_default(),
                p!("version").as_deref(),
                p!("scale").as_deref(),
                p!("format").as_deref(),
                p!("svg_outline_text").as_deref(),
                p!("svg_include_id").as_deref(),
                p!("svg_include_node_id").as_deref(),
                p!("svg_simplify_stroke").as_deref(),
                p!("contents_only").as_deref(),
                p!("use_absolute_bounds").as_deref(),
            )
            .await
        }
        "getImageFills" => {
            files::get_image_fills(token, &p!("file_key").unwrap_or_default()).await
        }
        "getFileMeta" => {
            files::get_file_meta(token, &p!("file_key").unwrap_or_default()).await
        }
        "getTeamProjects" => {
            projects::get_team_projects(token, &p!("team_id").unwrap_or_default()).await
        }
        "getProjectFiles" => {
            projects::get_project_files(
                token,
                &p!("project_id").unwrap_or_default(),
                p!("branch_data").as_deref(),
            )
            .await
        }
        "getProjectMeta" => {
            projects::get_project_meta(token, &p!("project_id").unwrap_or_default()).await
        }
        "getFileVersions" => {
            files::get_file_versions(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("page_size").as_deref(),
                p!("before").as_deref(),
                p!("after").as_deref(),
            )
            .await
        }
        "getComments" => {
            comments::get_comments(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("as_md").as_deref(),
            )
            .await
        }
        "postComment" => {
            let message = body_str(body, "message").or_else(|| p!("message"));
            let comment_id = body_str(body, "comment_id").or_else(|| p!("comment_id"));
            let client_meta = body_field(body, "client_meta").cloned();
            comments::post_comment(
                token,
                &p!("file_key").unwrap_or_default(),
                message.as_deref(),
                comment_id.as_deref(),
                client_meta.as_ref(),
            )
            .await
        }
        "deleteComment" => {
            comments::delete_comment(
                token,
                &p!("file_key").unwrap_or_default(),
                &p!("comment_id").unwrap_or_default(),
            )
            .await
        }
        "getCommentReactions" => {
            comments::get_comment_reactions(
                token,
                &p!("file_key").unwrap_or_default(),
                &p!("comment_id").unwrap_or_default(),
                p!("cursor").as_deref(),
            )
            .await
        }
        "postCommentReaction" => {
            let emoji = body_str(body, "emoji").or_else(|| p!("emoji"));
            comments::post_comment_reaction(
                token,
                &p!("file_key").unwrap_or_default(),
                &p!("comment_id").unwrap_or_default(),
                emoji.as_deref(),
            )
            .await
        }
        "deleteCommentReaction" => {
            let emoji = body_str(body, "emoji").or_else(|| p!("emoji"));
            comments::delete_comment_reaction(
                token,
                &p!("file_key").unwrap_or_default(),
                &p!("comment_id").unwrap_or_default(),
                emoji.as_deref(),
            )
            .await
        }
        "getMe" => users::get_me(token).await,
        "getTeamComponents" => {
            components::get_team_components(
                token,
                &p!("team_id").unwrap_or_default(),
                p!("page_size").as_deref(),
                p!("after").as_deref(),
                p!("before").as_deref(),
            )
            .await
        }
        "getFileComponents" => {
            components::get_file_components(token, &p!("file_key").unwrap_or_default()).await
        }
        "getComponent" => {
            components::get_component(token, &p!("key").unwrap_or_default()).await
        }
        "getTeamComponentSets" => {
            components::get_team_component_sets(
                token,
                &p!("team_id").unwrap_or_default(),
                p!("page_size").as_deref(),
                p!("after").as_deref(),
                p!("before").as_deref(),
            )
            .await
        }
        "getFileComponentSets" => {
            components::get_file_component_sets(token, &p!("file_key").unwrap_or_default()).await
        }
        "getComponentSet" => {
            components::get_component_set(token, &p!("key").unwrap_or_default()).await
        }
        "getTeamStyles" => {
            styles::get_team_styles(
                token,
                &p!("team_id").unwrap_or_default(),
                p!("page_size").as_deref(),
                p!("after").as_deref(),
                p!("before").as_deref(),
            )
            .await
        }
        "getFileStyles" => {
            styles::get_file_styles(token, &p!("file_key").unwrap_or_default()).await
        }
        "getStyle" => styles::get_style(token, &p!("key").unwrap_or_default()).await,
        "getWebhooks" => {
            webhooks::get_webhooks(
                token,
                p!("context").as_deref(),
                p!("plan").as_deref(),
                p!("after").as_deref(),
            )
            .await
        }
        "postWebhook" => webhooks::create_webhook(token, body).await,
        "getWebhook" => {
            webhooks::get_webhook(token, &p!("webhook_id").unwrap_or_default()).await
        }
        "putWebhook" => {
            webhooks::update_webhook(token, &p!("webhook_id").unwrap_or_default(), body).await
        }
        "deleteWebhook" => {
            webhooks::delete_webhook(token, &p!("webhook_id").unwrap_or_default()).await
        }
        "getTeamWebhooks" => {
            webhooks::get_team_webhooks(token, &p!("team_id").unwrap_or_default()).await
        }
        "getWebhookRequests" => {
            webhooks::get_webhook_requests(
                token,
                &p!("webhook_id").unwrap_or_default(),
                p!("cursor").as_deref(),
            )
            .await
        }
        "getActivityLogs" => {
            activity_logs::get_activity_logs(
                token,
                p!("start_time").as_deref(),
                p!("end_time").as_deref(),
                p!("events").as_deref(),
                p!("limit").as_deref(),
                p!("order").as_deref(),
            )
            .await
        }
        "getDeveloperLogs" => {
            let f = |bk: &str, pk: &str| body_str(body, bk).or_else(|| p!(pk));
            developer_logs::get_developer_logs(
                token,
                f("token_type", "token_type").as_deref(),
                body_str(body, "token").or_else(|| p!("token")).as_deref(),
                f("token_name", "token_name").as_deref(),
                f("user_email", "user_email").as_deref(),
                f("ip_address", "ip_address").as_deref(),
                f("event_source", "event_source").as_deref(),
                f("date_range", "date_range").as_deref(),
                body_str(body, "limit").or_else(|| p!("limit")).as_deref(),
                f("cursor", "cursor").as_deref(),
            )
            .await
        }
        "getPayments" => {
            payments::get_payments(
                token,
                p!("plugin_payment_token").as_deref(),
                p!("user_id").as_deref(),
                p!("community_file_id").as_deref(),
                p!("plugin_id").as_deref(),
                p!("widget_id").as_deref(),
            )
            .await
        }
        "getAiUsageDaily" => {
            ai_usage::get_ai_usage_daily(
                token,
                p!("start_date").as_deref(),
                p!("end_date").as_deref(),
                p!("user_email").as_deref(),
                p!("limit").as_deref(),
                p!("cursor").as_deref(),
            )
            .await
        }
        "getLocalVariables" => {
            variables::get_local_variables(token, &p!("file_key").unwrap_or_default()).await
        }
        "getPublishedVariables" => {
            variables::get_published_variables(token, &p!("file_key").unwrap_or_default()).await
        }
        "postVariables" => {
            variables::post_variables(token, &p!("file_key").unwrap_or_default(), body).await
        }
        "getDevResources" => {
            dev_resources::get_dev_resources(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("node_ids").as_deref(),
            )
            .await
        }
        "postDevResources" => {
            dev_resources::post_dev_resources(token, &dev_resources_body(body)).await
        }
        "putDevResources" => {
            dev_resources::put_dev_resources(token, &dev_resources_body(body)).await
        }
        "deleteDevResource" => {
            dev_resources::delete_dev_resource(
                token,
                &p!("file_key").unwrap_or_default(),
                &p!("dev_resource_id").unwrap_or_default(),
            )
            .await
        }
        "getLibraryAnalyticsComponentActions" => {
            analytics::component_actions(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("cursor").as_deref(),
                p!("start_date").as_deref(),
                p!("end_date").as_deref(),
                p!("order_direction").as_deref(),
            )
            .await
        }
        "getLibraryAnalyticsComponentUsages" => {
            analytics::component_usages(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("cursor").as_deref(),
                p!("start_date").as_deref(),
                p!("end_date").as_deref(),
                p!("order_direction").as_deref(),
            )
            .await
        }
        "getLibraryAnalyticsStyleActions" => {
            analytics::style_actions(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("cursor").as_deref(),
                p!("group_by").as_deref(),
                p!("start_date").as_deref(),
                p!("end_date").as_deref(),
            )
            .await
        }
        "getLibraryAnalyticsStyleUsages" => {
            analytics::style_usages(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("cursor").as_deref(),
                p!("start_date").as_deref(),
                p!("end_date").as_deref(),
                p!("order_direction").as_deref(),
            )
            .await
        }
        "getLibraryAnalyticsVariableActions" => {
            analytics::variable_actions(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("cursor").as_deref(),
                p!("start_date").as_deref(),
                p!("end_date").as_deref(),
                p!("order_direction").as_deref(),
            )
            .await
        }
        "getLibraryAnalyticsVariableUsages" => {
            analytics::variable_usages(
                token,
                &p!("file_key").unwrap_or_default(),
                p!("cursor").as_deref(),
                p!("start_date").as_deref(),
                p!("end_date").as_deref(),
                p!("order_direction").as_deref(),
            )
            .await
        }
        "getOembed" => {
            oembed::get_oembed(
                p!("url").as_deref(),
                p!("max_width").as_deref(),
                p!("max_height").as_deref(),
            )
            .await
        }
        other => Err(Error::Other(format!("Unknown Figma operationId: {other}"))),
    }
}

/// Wrap operation output in the standard fighorse envelope.
pub fn result_envelope(operation_id: &str, data: Value, ai_guidance: Option<Value>) -> Value {
    let op = operation(operation_id);
    let (operation_json, summary) = match &op {
        Some(o) => (
            o.summary_json(),
            format!("Figma {} {} completed.", o.method, o.path),
        ),
        None => (Value::Null, "Figma operation completed.".to_string()),
    };
    let guidance = ai_guidance.unwrap_or_else(|| {
        serde_json::json!({
            "summary": summary,
            "next_step": "Use the data directly, or call discover_fighorse/get_design_package when you need AI-optimized implementation context."
        })
    });
    serde_json::json!({
        "kind": "fighorse.api-result.v1",
        "operation": operation_json,
        "data": data,
        "ai_guidance": guidance,
    })
}
