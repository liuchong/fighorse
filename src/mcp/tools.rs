//! MCP Tools registry for Figma API.
//!
//! MCP tools registry: maps all Figma API endpoints plus AI
//! enhancements to MCP tools. Tool results are `{content: [{type, text}], ...}`
//! JSON objects; errors set `isError: true`.

use crate::api::{
    activity_logs as activity_logs_api, analytics as analytics_api,
    code_connect as code_connect_api, comments as comments_api, components as components_api,
    dev_resources as dev_resources_api, developer_logs as developer_logs_api, files as files_api,
    oembed as oembed_api, operations, payments as payments_api, projects as projects_api,
    styles as styles_api, users as users_api, variables as variables_api, webhooks as webhooks_api,
};
use crate::code_connect::model::CodeConnectDocument;
use crate::config;
use crate::discovery;
use crate::error::{Error, Result};
use crate::experience::{self, Filters, ScopeOpts};
use crate::export::images as img_export;
use crate::figma;
use crate::mcp::{policy, registry};
use crate::product::{design_package, playbook, resource_catalog, visual_audit};
use crate::transform::{compact, tokens};
use crate::url as figma_url;
use serde_json::{Value, json};
use std::collections::HashSet;

const MISSING_TOKEN_MESSAGE: &str = "fighorse needs a Figma Personal Access Token before calling Figma APIs. Run `fighorse auth login --token <FIGMA_TOKEN>` or set FIGMA_TOKEN, then call check_fighorse_ready again.";

/// The base read-only tool definitions (files, projects, users, etc.).
fn base_tools() -> Value {
    // Loaded from a static JSON blob shipped with the binary.
    serde_json::from_str(include_str!("tools_base.json")).expect("valid base tools json")
}

fn extra_tools() -> Value {
    serde_json::from_str(include_str!("tools_extra.json")).expect("valid extra tools json")
}

fn write_tools() -> Value {
    serde_json::from_str(include_str!("tools_write.json")).expect("valid write tools json")
}

fn write_tool_names() -> HashSet<String> {
    write_tools()
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Successful MCP tool result wrapping pretty JSON text.
fn success(data: &Value) -> Value {
    let text = serde_json::to_string_pretty(data).unwrap_or_else(|_| "null".into());
    json!({"content": [{"type": "text", "text": text}]})
}

/// Error MCP tool result.
fn error(msg: &str) -> Value {
    json!({"content": [{"type": "text", "text": format!("Error: {msg}")}], "isError": true})
}

fn get_token() -> Result<String> {
    match config::load_config().token {
        Some(t) if !t.trim().is_empty() => Ok(t),
        _ => Err(Error::Other(MISSING_TOKEN_MESSAGE.to_string())),
    }
}

/// Convert a Result<Value> into an MCP tool result.
fn handle(result: Result<Value>) -> Value {
    match result {
        Ok(data) => success(&data),
        Err(e) => error(&e.to_string()),
    }
}

fn code_connect_documents(args: &Value) -> Result<Vec<CodeConnectDocument>> {
    serde_json::from_value(args.get("documents").cloned().unwrap_or(json!([]))).map_err(Error::from)
}

fn code_connect_nodes(args: &Value) -> Result<Vec<(String, String)>> {
    let nodes = args
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::Usage("nodes array required".into()))?;
    let mut out = Vec::new();
    for node in nodes {
        let figma_node = node
            .get("figmaNode")
            .or_else(|| node.get("figma_node"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Usage("nodes[].figmaNode required".into()))?;
        let label = node
            .get("label")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Usage("nodes[].label required".into()))?;
        out.push((figma_node.to_string(), label.to_string()));
    }
    Ok(out)
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn arg_i64(args: &Value, key: &str) -> Option<i64> {
    match args.get(key) {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

fn arg_f64(args: &Value, key: &str) -> Option<f64> {
    match args.get(key) {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

fn arg_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn strict_bool(args: &Value, key: &str, default: bool) -> Result<bool> {
    match args.get(key) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(Error::Usage(format!("{key} must be a boolean"))),
    }
}

fn int_str(v: Option<i64>) -> Option<String> {
    v.map(|n| n.to_string())
}

fn num_str(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Return the tool list for `tools/list`, honoring write-mode gating.
pub fn list_tools() -> Value {
    let write_enabled = config::mcp_write_enabled();
    let mut tools: Vec<Value> = Vec::new();

    if let Some(arr) = base_tools().as_array() {
        tools.extend(arr.iter().cloned());
    }
    if let Some(arr) = extra_tools().as_array() {
        tools.extend(arr.iter().cloned());
    }

    // Official operation tools (filtered when readonly).
    for t in registry::official_tools() {
        let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if write_enabled || !registry::write_tool_name(name) {
            tools.push(t);
        }
    }

    if write_enabled {
        if let Some(arr) = write_tools().as_array() {
            tools.extend(arr.iter().cloned());
        }
    }

    // select-keys [:name :description :inputSchema]
    let selected: Vec<Value> = tools
        .iter()
        .map(|t| {
            let mut m = serde_json::Map::new();
            for k in ["name", "description", "inputSchema"] {
                if let Some(v) = t.get(k) {
                    m.insert(k.to_string(), v.clone());
                }
            }
            Value::Object(m)
        })
        .collect();

    json!({ "tools": selected })
}

/// Execute a tool call for `tools/call`.
pub async fn call_tool(name: &str, args: &Value) -> Value {
    // Policy gating first.
    if let Some(msg) = policy::violation(&write_tool_names(), name) {
        return error(&msg);
    }

    // Official figma_* operations route through the operations dispatcher.
    if registry::official_tool_name(name) {
        let operation_id = match registry::operation_id_for_tool(name) {
            Some(id) => id,
            None => return error(&format!("Unknown tool: {name}")),
        };
        let token = match get_token() {
            Ok(t) => t,
            Err(e) => return error(&e.to_string()),
        };
        let params = args.get("params").cloned().unwrap_or(json!({}));
        let body = args.get("body").cloned().unwrap_or(json!({}));
        let result = operations::call_operation(&token, &operation_id, &params, &body).await;
        return match result {
            Ok(data) => {
                if arg_bool(args, "ai_guidance").unwrap_or(false) {
                    success(&operations::result_envelope(&operation_id, data, None))
                } else {
                    success(&data)
                }
            }
            Err(e) => error(&e.to_string()),
        };
    }

    handle_tool(name, args).await
}

async fn handle_tool(name: &str, args: &Value) -> Value {
    match name {
        // --- Self discovery / AI replication ---
        "discover_fighorse" => success(&discovery::manifest()),
        "check_fighorse_ready" => success(&discovery::doctor()),
        "parse_figma_url" => {
            success(&figma_url::parse_figma_url(arg_str(args, "figma_url").unwrap_or("")).to_json())
        }
        "get_replicate_workflow" => success(&discovery::workflow()),
        "get_experience_schema" => success(&experience::schema()),
        "list_experiences" => {
            let filters = Filters {
                platform: arg_str(args, "platform").map(String::from),
                asset_format: arg_str(args, "asset_format").map(String::from),
                category: arg_str(args, "category").map(String::from),
                tag: arg_str(args, "tag").map(String::from),
            };
            let opts = ScopeOpts {
                scope: arg_str(args, "scope").map(String::from),
                project_dir: arg_str(args, "project_dir").map(String::from),
            };
            let limit = arg_i64(args, "limit").unwrap_or(6) as usize;
            success(&experience::guidance(&filters, limit, &opts))
        }
        "record_experience" => {
            let opts = ScopeOpts {
                scope: arg_str(args, "scope").map(String::from),
                project_dir: arg_str(args, "project_dir").map(String::from),
            };
            match experience::add(args, &opts) {
                Ok(v) => success(&v),
                Err(e) => error(&e.to_string()),
            }
        }
        "get_design_package" => handle_design_package(args).await,
        "get_resource_catalog" => handle_resource_catalog(args).await,
        "visual_audit" => success(&visual_audit::audit(
            arg_str(args, "figma_url"),
            arg_str(args, "screenshot_path"),
            arg_str(args, "platform"),
            arg_str(args, "asset_format"),
            arg_str(args, "notes"),
        )),
        "get_project_playbook" => success(&playbook::build(
            arg_str(args, "platform"),
            arg_str(args, "asset_format"),
            arg_str(args, "project_dir"),
        )),
        "parse_code_connect_template" => match code_connect_documents(args) {
            Ok(docs) => success(&json!({"documents": docs})),
            Err(e) => error(&e.to_string()),
        },
        "validate_code_connect" => {
            let token = match get_token() {
                Ok(t) => t,
                Err(e) => return error(&e.to_string()),
            };
            match code_connect_documents(args) {
                Ok(docs) => handle(
                    code_connect_api::validate_documents(&token, &docs)
                        .await
                        .map(|report| serde_json::to_value(report).unwrap_or(json!({}))),
                ),
                Err(e) => error(&e.to_string()),
            }
        }
        "preview_code_connect" => {
            let token = match get_token() {
                Ok(t) => t,
                Err(e) => return error(&e.to_string()),
            };
            match code_connect_documents(args) {
                Ok(docs) => handle(
                    code_connect_api::preview_documents(
                        &token,
                        &docs,
                        args.get("render_combinations").cloned(),
                    )
                    .await,
                ),
                Err(e) => error(&e.to_string()),
            }
        }
        "publish_code_connect" => {
            if !arg_bool(args, "yes").unwrap_or(false) {
                return error(
                    "publish_code_connect requires yes=true after reviewing the publish plan",
                );
            }
            let token = match get_token() {
                Ok(t) => t,
                Err(e) => return error(&e.to_string()),
            };
            match code_connect_documents(args) {
                Ok(docs) => handle(
                    code_connect_api::publish_documents(
                        &token,
                        &docs,
                        arg_bool(args, "force").unwrap_or(false),
                        None,
                    )
                    .await,
                ),
                Err(e) => error(&e.to_string()),
            }
        }
        "unpublish_code_connect" => {
            if !arg_bool(args, "yes").unwrap_or(false) {
                return error(
                    "unpublish_code_connect requires yes=true after reviewing the delete plan",
                );
            }
            let token = match get_token() {
                Ok(t) => t,
                Err(e) => return error(&e.to_string()),
            };
            match code_connect_nodes(args) {
                Ok(nodes) => {
                    let borrowed: Vec<(&str, &str)> = nodes
                        .iter()
                        .map(|(figma_node, label)| (figma_node.as_str(), label.as_str()))
                        .collect();
                    handle(code_connect_api::unpublish_documents(&token, &borrowed).await)
                }
                Err(e) => error(&e.to_string()),
            }
        }

        // --- Files ---
        "get_file" => handle(
            with_token(|t| async move {
                files_api::get_file(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    files_api::GetFileParams {
                        version: arg_str(args, "version"),
                        depth: int_str(arg_i64(args, "depth")).as_deref(),
                        ..Default::default()
                    },
                )
                .await
            })
            .await,
        ),
        "get_file_nodes" | "get_node" => {
            let node = arg_str(args, "node_ids")
                .or_else(|| arg_str(args, "node_id"))
                .unwrap_or("");
            handle(
                with_token(|t| async move {
                    files_api::get_file_nodes(
                        &t,
                        arg_str(args, "file_key").unwrap_or(""),
                        node,
                        None,
                        int_str(arg_i64(args, "depth")).as_deref(),
                        None,
                        None,
                    )
                    .await
                })
                .await,
            )
        }
        "get_file_compact" | "get_design_context" => handle_file_compact(args).await,
        "get_tokens" => handle_get_tokens(args).await,
        "get_file_tree" => handle_file_tree(args).await,
        "get_file_meta" => handle(
            with_token(|t| async move {
                files_api::get_file_meta(&t, arg_str(args, "file_key").unwrap_or("")).await
            })
            .await,
        ),
        "get_file_versions" => handle(
            with_token(|t| async move {
                files_api::get_file_versions(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    int_str(arg_i64(args, "page_size")).as_deref(),
                    None,
                    None,
                )
                .await
            })
            .await,
        ),
        "get_images" | "get_screenshot" => {
            let scale = num_str(arg_f64(args, "scale").unwrap_or(2.0));
            let format = arg_str(args, "format").unwrap_or("png").to_string();
            handle(
                with_token(|t| async move {
                    files_api::get_images(
                        &t,
                        arg_str(args, "file_key").unwrap_or(""),
                        arg_str(args, "node_ids").unwrap_or(""),
                        None,
                        Some(&scale),
                        Some(&format),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                    .await
                })
                .await,
            )
        }
        "get_image_fills" => handle(
            with_token(|t| async move {
                files_api::get_image_fills(&t, arg_str(args, "file_key").unwrap_or("")).await
            })
            .await,
        ),
        "export_images" | "export_component" => handle_export_images(args).await,
        "download_image_fills" => handle_download_image_fills(args).await,

        // --- Projects ---
        "get_team_projects" => handle(
            with_token(|t| async move {
                projects_api::get_team_projects(&t, arg_str(args, "team_id").unwrap_or("")).await
            })
            .await,
        ),
        "get_project_files" => handle(
            with_token(|t| async move {
                projects_api::get_project_files(&t, arg_str(args, "project_id").unwrap_or(""), None)
                    .await
            })
            .await,
        ),

        // --- Users ---
        "get_me" => handle(with_token(|t| async move { users_api::get_me(&t).await }).await),

        // --- Components ---
        "get_team_components" => handle(
            with_token(|t| async move {
                components_api::get_team_components(
                    &t,
                    arg_str(args, "team_id").unwrap_or(""),
                    int_str(arg_i64(args, "page_size")).as_deref(),
                    None,
                    None,
                )
                .await
            })
            .await,
        ),
        "get_file_components" => handle(
            with_token(|t| async move {
                components_api::get_file_components(&t, arg_str(args, "file_key").unwrap_or(""))
                    .await
            })
            .await,
        ),
        "get_component" => handle(
            with_token(|t| async move {
                components_api::get_component(&t, arg_str(args, "component_key").unwrap_or(""))
                    .await
            })
            .await,
        ),
        "get_file_component_sets" => handle(
            with_token(|t| async move {
                components_api::get_file_component_sets(&t, arg_str(args, "file_key").unwrap_or(""))
                    .await
            })
            .await,
        ),
        "get_team_component_sets" => handle(
            with_token(|t| async move {
                components_api::get_team_component_sets(
                    &t,
                    arg_str(args, "team_id").unwrap_or(""),
                    int_str(arg_i64(args, "page_size")).as_deref(),
                    None,
                    None,
                )
                .await
            })
            .await,
        ),
        "get_component_set" => handle(
            with_token(|t| async move {
                components_api::get_component_set(
                    &t,
                    arg_str(args, "component_set_key").unwrap_or(""),
                )
                .await
            })
            .await,
        ),

        // --- Styles ---
        "get_team_styles" => handle(
            with_token(|t| async move {
                styles_api::get_team_styles(
                    &t,
                    arg_str(args, "team_id").unwrap_or(""),
                    int_str(arg_i64(args, "page_size")).as_deref(),
                    None,
                    None,
                )
                .await
            })
            .await,
        ),
        "get_file_styles" => handle(
            with_token(|t| async move {
                styles_api::get_file_styles(&t, arg_str(args, "file_key").unwrap_or("")).await
            })
            .await,
        ),
        "get_style" => handle(
            with_token(|t| async move {
                styles_api::get_style(&t, arg_str(args, "style_key").unwrap_or("")).await
            })
            .await,
        ),

        // --- Comments ---
        "get_comments" => {
            let as_md = arg_bool(args, "as_md").map(|b| b.to_string());
            handle(
                with_token(|t| async move {
                    comments_api::get_comments(
                        &t,
                        arg_str(args, "file_key").unwrap_or(""),
                        as_md.as_deref(),
                    )
                    .await
                })
                .await,
            )
        }
        "post_comment" => handle(
            with_token(|t| async move {
                comments_api::post_comment(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    arg_str(args, "message"),
                    arg_str(args, "reply_to"),
                    None,
                )
                .await
            })
            .await,
        ),
        "delete_comment" => handle(
            with_token(|t| async move {
                comments_api::delete_comment(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    arg_str(args, "comment_id").unwrap_or(""),
                )
                .await
            })
            .await,
        ),
        "get_comment_reactions" => handle(
            with_token(|t| async move {
                comments_api::get_comment_reactions(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    arg_str(args, "comment_id").unwrap_or(""),
                    arg_str(args, "cursor"),
                )
                .await
            })
            .await,
        ),
        "post_comment_reaction" => handle(
            with_token(|t| async move {
                comments_api::post_comment_reaction(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    arg_str(args, "comment_id").unwrap_or(""),
                    arg_str(args, "emoji"),
                )
                .await
            })
            .await,
        ),
        "delete_comment_reaction" => handle(
            with_token(|t| async move {
                comments_api::delete_comment_reaction(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    arg_str(args, "comment_id").unwrap_or(""),
                    arg_str(args, "emoji"),
                )
                .await
            })
            .await,
        ),

        // --- Variables ---
        "get_local_variables" => handle(
            with_token(|t| async move {
                variables_api::get_local_variables(&t, arg_str(args, "file_key").unwrap_or(""))
                    .await
            })
            .await,
        ),
        "get_published_variables" => handle(
            with_token(|t| async move {
                variables_api::get_published_variables(&t, arg_str(args, "file_key").unwrap_or(""))
                    .await
            })
            .await,
        ),
        "post_variables" => {
            let changes = args.get("changes").cloned().unwrap_or(json!({}));
            handle(
                with_token(|t| async move {
                    variables_api::post_variables(
                        &t,
                        arg_str(args, "file_key").unwrap_or(""),
                        &changes,
                    )
                    .await
                })
                .await,
            )
        }

        // --- Dev Resources ---
        "get_dev_resources" => handle(
            with_token(|t| async move {
                dev_resources_api::get_dev_resources(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    arg_str(args, "node_ids"),
                )
                .await
            })
            .await,
        ),
        "post_dev_resources" => {
            let dr = args.get("dev_resources").cloned().unwrap_or(json!([]));
            handle(
                with_token(|t| async move { dev_resources_api::post_dev_resources(&t, &dr).await })
                    .await,
            )
        }
        "put_dev_resources" => {
            let dr = args.get("dev_resources").cloned().unwrap_or(json!([]));
            handle(
                with_token(|t| async move { dev_resources_api::put_dev_resources(&t, &dr).await })
                    .await,
            )
        }
        "create_dev_resource" => {
            let dr = json!([{
                "name": arg_str(args, "name"),
                "url": arg_str(args, "url"),
                "file_key": arg_str(args, "file_key"),
                "node_id": arg_str(args, "node_id"),
            }]);
            handle(
                with_token(|t| async move { dev_resources_api::post_dev_resources(&t, &dr).await })
                    .await,
            )
        }
        "delete_dev_resource" => handle(
            with_token(|t| async move {
                dev_resources_api::delete_dev_resource(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    arg_str(args, "dev_resource_id").unwrap_or(""),
                )
                .await
            })
            .await,
        ),

        // --- Webhooks ---
        "get_webhooks" => handle(
            with_token(|t| async move {
                webhooks_api::get_webhooks(&t, arg_str(args, "context"), None, None).await
            })
            .await,
        ),
        "get_webhook" => handle(
            with_token(|t| async move {
                webhooks_api::get_webhook(&t, arg_str(args, "webhook_id").unwrap_or("")).await
            })
            .await,
        ),
        "get_team_webhooks" => handle(
            with_token(|t| async move {
                webhooks_api::get_team_webhooks(&t, arg_str(args, "team_id").unwrap_or("")).await
            })
            .await,
        ),
        "get_webhook_requests" => handle(
            with_token(|t| async move {
                webhooks_api::get_webhook_requests(
                    &t,
                    arg_str(args, "webhook_id").unwrap_or(""),
                    arg_str(args, "cursor"),
                )
                .await
            })
            .await,
        ),
        "create_webhook" => {
            let body = json!({
                "event_type": arg_str(args, "event_type"),
                "team_id": arg_str(args, "team_id"),
                "endpoint": arg_str(args, "endpoint"),
                "passcode": arg_str(args, "passcode"),
                "description": arg_str(args, "description"),
                "status": arg_str(args, "status"),
            });
            handle(
                with_token(|t| async move { webhooks_api::create_webhook(&t, &body).await }).await,
            )
        }
        "update_webhook" => {
            let webhook = args.get("webhook").cloned().unwrap_or(json!({}));
            handle(
                with_token(|t| async move {
                    webhooks_api::update_webhook(
                        &t,
                        arg_str(args, "webhook_id").unwrap_or(""),
                        &webhook,
                    )
                    .await
                })
                .await,
            )
        }
        "delete_webhook" => handle(
            with_token(|t| async move {
                webhooks_api::delete_webhook(&t, arg_str(args, "webhook_id").unwrap_or("")).await
            })
            .await,
        ),

        // --- Admin / Analytics / Payments / oEmbed ---
        "get_activity_logs" => handle(
            with_token(|t| async move {
                activity_logs_api::get_activity_logs(
                    &t,
                    arg_str(args, "start_time"),
                    arg_str(args, "end_time"),
                    arg_str(args, "events"),
                    int_str(arg_i64(args, "limit")).as_deref(),
                    arg_str(args, "order"),
                )
                .await
            })
            .await,
        ),
        "get_developer_logs" => handle(
            with_token(|t| async move {
                developer_logs_api::get_developer_logs(
                    &t,
                    arg_str(args, "token_type"),
                    arg_str(args, "token"),
                    arg_str(args, "token_name"),
                    arg_str(args, "user_email"),
                    arg_str(args, "ip_address"),
                    arg_str(args, "event_source"),
                    arg_str(args, "date_range"),
                    int_str(arg_i64(args, "limit")).as_deref(),
                    arg_str(args, "cursor"),
                )
                .await
            })
            .await,
        ),
        "get_payments" => handle(
            with_token(|t| async move {
                payments_api::get_payments(
                    &t,
                    arg_str(args, "plugin_payment_token"),
                    arg_str(args, "user_id"),
                    arg_str(args, "community_file_id"),
                    arg_str(args, "plugin_id"),
                    arg_str(args, "widget_id"),
                )
                .await
            })
            .await,
        ),
        "get_oembed" => handle(
            with_token(|_t| async move {
                oembed_api::get_oembed(
                    arg_str(args, "url"),
                    int_str(arg_i64(args, "max_width")).as_deref(),
                    int_str(arg_i64(args, "max_height")).as_deref(),
                )
                .await
            })
            .await,
        ),
        "get_library_analytics_component_usages" => handle(
            with_token(|t| async move {
                analytics_api::component_usages(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    None,
                    None,
                    None,
                    None,
                )
                .await
            })
            .await,
        ),
        "get_library_analytics_component_actions" => handle(
            with_token(|t| async move {
                analytics_api::component_actions(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    None,
                    None,
                    None,
                    None,
                )
                .await
            })
            .await,
        ),
        "get_library_analytics_style_usages" => handle(
            with_token(|t| async move {
                analytics_api::style_usages(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    None,
                    None,
                    None,
                    None,
                )
                .await
            })
            .await,
        ),
        "get_library_analytics_style_actions" => handle(
            with_token(|t| async move {
                analytics_api::style_actions(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    None,
                    arg_str(args, "group_by"),
                    None,
                    None,
                )
                .await
            })
            .await,
        ),
        "get_library_analytics_variable_usages" => handle(
            with_token(|t| async move {
                analytics_api::variable_usages(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    None,
                    None,
                    None,
                    None,
                )
                .await
            })
            .await,
        ),
        "get_library_analytics_variable_actions" => handle(
            with_token(|t| async move {
                analytics_api::variable_actions(
                    &t,
                    arg_str(args, "file_key").unwrap_or(""),
                    None,
                    None,
                    None,
                    None,
                )
                .await
            })
            .await,
        ),

        other => error(&format!("Unknown tool: {other}")),
    }
}

/// Run an async closure with the resolved token, converting missing-token to an
/// error Result. This keeps the `?`-free dispatch above readable.
async fn with_token<F, Fut>(f: F) -> Result<Value>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<Value>>,
{
    let token = get_token()?;
    f(token).await
}

async fn handle_design_package(args: &Value) -> Value {
    let token = match get_token() {
        Ok(t) => t,
        Err(e) => return error(&e.to_string()),
    };
    let opts = design_package::PackageOpts {
        figma_url: arg_str(args, "figma_url"),
        file_key: arg_str(args, "file_key"),
        node_id: arg_str(args, "node_id"),
        depth: arg_i64(args, "depth").unwrap_or(2),
        max_tokens: arg_i64(args, "max_tokens").unwrap_or(8000),
        include_screenshot: arg_bool(args, "include_screenshot").unwrap_or(true),
        include_assets: arg_bool(args, "include_assets").unwrap_or(false),
        screenshot_format: arg_str(args, "screenshot_format")
            .unwrap_or("png")
            .to_string(),
        scale: arg_f64(args, "scale").unwrap_or(2.0),
        screenshot_limit: 4,
        platform: arg_str(args, "platform"),
        asset_format: arg_str(args, "asset_format"),
    };
    handle(design_package::get_design_package(&token, opts).await)
}

async fn handle_resource_catalog(args: &Value) -> Value {
    let max_probes = match args.get("max_probes") {
        Some(_) => match arg_i64(args, "max_probes") {
            Some(value) => value,
            None => return error("max_probes must be an integer"),
        },
        None => 25,
    };
    if max_probes < 0 {
        return error("max_probes must be zero or greater");
    }
    let include_libraries = match strict_bool(args, "include_libraries", true) {
        Ok(value) => value,
        Err(err) => return error(&err.to_string()),
    };
    let probe_file_access = match strict_bool(args, "probe_file_access", false) {
        Ok(value) => value,
        Err(err) => return error(&err.to_string()),
    };
    let opts = resource_catalog::CatalogOpts {
        figma_url: arg_str(args, "figma_url"),
        team_id: arg_str(args, "team_id"),
        project_id: arg_str(args, "project_id"),
        include_libraries,
        branch_data: true,
        probe_file_access,
        max_probes: max_probes as usize,
    };
    match resource_catalog::local_catalog_outcome(&opts) {
        Ok(Some(outcome)) => return success(&outcome.report),
        Ok(None) => {}
        Err(err) => return error(&err.to_string()),
    }
    let token = match get_token() {
        Ok(token) => token,
        Err(err) => return error(&err.to_string()),
    };
    match resource_catalog::get_resource_catalog(&token, opts).await {
        Ok(outcome) => success(&outcome.report),
        Err(error_value) => error(&error_value.to_string()),
    }
}

async fn handle_file_compact(args: &Value) -> Value {
    let token = match get_token() {
        Ok(t) => t,
        Err(e) => return error(&e.to_string()),
    };
    let depth = arg_i64(args, "depth");
    let max_tokens = arg_i64(args, "max_tokens");
    let file_key = arg_str(args, "file_key").unwrap_or("").to_string();
    match files_api::get_file(
        &token,
        &file_key,
        files_api::GetFileParams {
            depth: int_str(depth).as_deref(),
            ..Default::default()
        },
    )
    .await
    {
        Ok(data) => {
            let node = figma::response_to_node(&data);
            success(&compact::compact(&node, depth, max_tokens))
        }
        Err(e) => error(&e.to_string()),
    }
}

async fn handle_get_tokens(args: &Value) -> Value {
    let token = match get_token() {
        Ok(t) => t,
        Err(e) => return error(&e.to_string()),
    };
    let depth = arg_i64(args, "depth");
    let effective = depth.unwrap_or(2);
    let file_key = arg_str(args, "file_key").unwrap_or("").to_string();
    match files_api::get_file(
        &token,
        &file_key,
        files_api::GetFileParams {
            depth: Some(&effective.to_string()),
            ..Default::default()
        },
    )
    .await
    {
        Ok(data) => {
            let doc = data.get("document").cloned().unwrap_or(Value::Null);
            let simplified = compact::simplify_tree(&doc, depth);
            let extracted = tokens::extract_tokens(&simplified);
            success(&tokens::tokens_by_category(&extracted))
        }
        Err(e) => error(&e.to_string()),
    }
}

async fn handle_file_tree(args: &Value) -> Value {
    let token = match get_token() {
        Ok(t) => t,
        Err(e) => return error(&e.to_string()),
    };
    let depth = arg_i64(args, "depth");
    let effective = depth.unwrap_or(2);
    let file_key = arg_str(args, "file_key").unwrap_or("").to_string();
    match files_api::get_file(
        &token,
        &file_key,
        files_api::GetFileParams {
            depth: Some(&effective.to_string()),
            ..Default::default()
        },
    )
    .await
    {
        Ok(data) => {
            let doc = data.get("document").cloned().unwrap_or(Value::Null);
            let tree = compact::simplify_tree_with(&doc, depth, compact::TREE_EXTRACTORS);
            success(&tree)
        }
        Err(e) => error(&e.to_string()),
    }
}

async fn handle_export_images(args: &Value) -> Value {
    let token = match get_token() {
        Ok(t) => t,
        Err(e) => return error(&e.to_string()),
    };
    let node_ids: Vec<String> = arg_str(args, "node_ids")
        .unwrap_or("")
        .split(',')
        .map(String::from)
        .collect();
    let format = arg_str(args, "format").unwrap_or("png");
    let scale = num_str(arg_f64(args, "scale").unwrap_or(2.0));
    let manifest = arg_bool(args, "manifest").unwrap_or(false);
    match img_export::export_images(
        &token,
        arg_str(args, "file_key").unwrap_or(""),
        &node_ids,
        &img_export::ExportOptions {
            format,
            scale: &scale,
            dest_dir: arg_str(args, "dest_dir"),
            manifest,
            prefix: arg_str(args, "prefix"),
        },
    )
    .await
    {
        Ok(rows) => success(&rows_to_map(rows)),
        Err(e) => error(&e.to_string()),
    }
}

async fn handle_download_image_fills(args: &Value) -> Value {
    let token = match get_token() {
        Ok(t) => t,
        Err(e) => return error(&e.to_string()),
    };
    let manifest = arg_bool(args, "manifest").unwrap_or(false);
    match img_export::download_image_fills(
        &token,
        arg_str(args, "file_key").unwrap_or(""),
        arg_str(args, "dest_dir"),
        manifest,
        arg_str(args, "prefix"),
    )
    .await
    {
        Ok(rows) => success(&rows_to_map(rows)),
        Err(e) => error(&e.to_string()),
    }
}

fn rows_to_map(rows: Vec<(String, String)>) -> Value {
    let mut m = serde_json::Map::new();
    for (k, v) in rows {
        m.insert(k, Value::String(v));
    }
    Value::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definitions_parse() {
        assert!(base_tools().as_array().is_some());
        assert!(extra_tools().as_array().is_some());
        assert!(write_tools().as_array().is_some());
    }

    #[test]
    fn list_tools_readonly_excludes_write() {
        unsafe { std::env::set_var("FIGHORSE_MCP_MODE", "readonly") };
        let tools = list_tools();
        let names: HashSet<String> = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
            .collect();
        assert!(names.contains("discover_fighorse"));
        assert!(names.contains("get_file"));
        assert!(names.contains("figma_get_file"));
        // Write tools absent in readonly.
        assert!(!names.contains("post_comment"));
        assert!(!names.contains("figma_post_comment"));
        unsafe { std::env::remove_var("FIGHORSE_MCP_MODE") };
    }
}
