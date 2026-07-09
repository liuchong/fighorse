//! CLI command implementations (data, auth, assets, transforms).
//!
//! Each `cmd_*` mirrors the corresponding handler in `fighorse.core`. Async
//! handlers return `Result<()>`; errors are printed and turned into exit code 1
//! by the dispatcher in `main`.

use super::args::{flag_present, optional_float, optional_int, parse_flags};
use super::{
    err_message, parse_json_map, print_data, read_stdin, require_arg, require_token,
    require_value, write_output,
};
use crate::api::{
    coverage as api_coverage, comments as comments_api, components as components_api,
    dev_resources as dev_resources_api, files as files_api, operations as api_operations,
    projects as projects_api, styles as styles_api, users as users_api, variables as variables_api,
    webhooks as webhooks_api,
};
use crate::config;
use crate::error::Result;
use crate::export::{images as img_export, md as md_export};
use crate::figma;
use crate::transform::{compact, filter as tree_filter, schema, tokens};
use crate::url as figma_url;
use serde_json::{json, Value};

fn int_str(v: i64) -> String {
    v.to_string()
}

// --- Auth ---

pub fn cmd_auth_login(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--token"]);
    let raw_token = flags
        .get("token")
        .map(String::from)
        .or_else(|| flags.arg(0).map(String::from))
        .or_else(|| {
            // stdin fallback when not a TTY.
            if atty_stdin() {
                None
            } else {
                Some(read_stdin())
            }
        });
    let token = raw_token.map(|t| t.trim().to_string());
    match token {
        Some(t) if !t.is_empty() => {
            config::save_config(&json!({ "token": t }))?;
            println!("Saved Figma token to {}", config::config_path().display());
            Ok(())
        }
        _ => {
            eprintln!("Error: token required. Use `fighorse auth login --token <token>` or pipe token on stdin.");
            std::process::exit(1);
        }
    }
}

pub fn cmd_auth_logout(_args: &[String]) -> Result<()> {
    config::clear_config()?;
    println!("Removed saved Figma token");
    Ok(())
}

pub fn cmd_auth_status(_args: &[String]) -> Result<()> {
    let cfg = config::load_config();
    let path = cfg.config_path.display();
    match cfg.token {
        Some(t) if !t.trim().is_empty() => println!("Authenticated. Config path: {path}"),
        _ => println!("Not authenticated. Config path: {path}"),
    }
    Ok(())
}

fn atty_stdin() -> bool {
    // Best-effort TTY check without extra deps.
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        unsafe { libc_isatty(std::io::stdin().as_raw_fd()) }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(unix)]
unsafe fn libc_isatty(fd: i32) -> bool {
    extern "C" {
        fn isatty(fd: i32) -> i32;
    }
    isatty(fd) == 1
}

// --- File commands ---

pub async fn cmd_file_get(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--version", "--depth", "--ids", "--geometry", "--output"]);
    let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
    let depth = flags
        .get("depth")
        .map(String::from)
        .or_else(|| flags.arg(1).map(String::from));
    let data = files_api::get_file(
        &token,
        &file_key,
        flags.get("version"),
        flags.get("ids"),
        depth.as_deref(),
        flags.get("geometry"),
        None,
        None,
    )
    .await?;
    print_data(&data, flags.get("output"))
}

pub async fn cmd_file_nodes(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let ids = require_arg(args, 1, "node-ids").to_string();
    let rest: Vec<String> = args.iter().skip(2).cloned().collect();
    let flags = parse_flags(&rest, &["--depth"]);
    let data = files_api::get_file_nodes(
        &token,
        &file_key,
        &ids,
        None,
        flags.get("depth"),
        None,
        None,
    )
    .await?;
    print_data(&data, None)
}

pub async fn cmd_file_meta(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let data = files_api::get_file_meta(&token, &file_key).await?;
    print_data(&data, None)
}

pub async fn cmd_file_versions(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let flags = parse_flags(&rest, &["--page-size"]);
    let data = files_api::get_file_versions(&token, &file_key, flags.get("page_size"), None, None).await?;
    print_data(&data, None)
}

pub async fn cmd_file_compact(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--max-tokens", "--depth", "--ids", "--output"]);
    let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
    let depth = optional_int(flags.get("depth")).or_else(|| optional_int(flags.arg(1)));
    let max_tokens = optional_int(flags.get("max_tokens"));
    let data = files_api::get_file(
        &token,
        &file_key,
        None,
        flags.get("ids"),
        depth.map(int_str).as_deref(),
        None,
        None,
        None,
    )
    .await?;
    let node = figma::response_to_node(&data);
    let compacted = compact::compact(&node, depth, max_tokens);
    print_data(&compacted, flags.get("output"))
}

pub fn cmd_compact_stdin(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--max-tokens", "--depth", "--output"]);
    let input: Value = serde_json::from_str(&read_stdin())?;
    let node = figma::response_to_node(&input);
    let depth = optional_int(flags.get("depth"));
    let max_tokens = optional_int(flags.get("max_tokens"));
    let compacted = compact::compact(&node, depth, max_tokens);
    print_data(&compacted, flags.get("output"))
}

pub async fn cmd_file_tree(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--depth", "--max-depth", "--output"]);
    let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
    let depth = optional_int(flags.get("depth"))
        .or_else(|| optional_int(flags.get("max_depth")))
        .or_else(|| optional_int(flags.arg(1)));
    let effective_depth = depth.unwrap_or(2);
    let data = files_api::get_file(
        &token,
        &file_key,
        None,
        None,
        Some(&int_str(effective_depth)),
        None,
        None,
        None,
    )
    .await?;
    let doc = data.get("document").cloned().unwrap_or(Value::Null);
    // file tree uses only dimension + layout extractors.
    let tree = compact::simplify_tree_with(&doc, depth, &compact::TREE_EXTRACTORS);
    print_data(&tree, flags.get("output"))
}

pub async fn cmd_file_to_md(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--title", "--depth", "--output"]);
    let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
    let depth = optional_int(flags.get("depth")).or_else(|| optional_int(flags.arg(1)));
    let include_tokens = flag_present(args, "--include-tokens");
    let include_screenshots = flag_present(args, "--include-screenshots");
    let effective_depth = depth.unwrap_or(2);

    let data = files_api::get_file(
        &token,
        &file_key,
        None,
        None,
        Some(&int_str(effective_depth)),
        None,
        None,
        None,
    )
    .await?;
    let doc = data.get("document").cloned().unwrap_or(Value::Null);
    let simplified = compact::simplify_tree(&doc, depth);
    let title = flags
        .get("title")
        .map(String::from)
        .or_else(|| doc.get("name").and_then(|v| v.as_str()).map(String::from));
    let mut md = md_export::tree_to_markdown(&simplified, title.as_deref());

    if include_tokens {
        let extracted = tokens::extract_tokens(&simplified);
        let by_cat = tokens::tokens_by_category(&extracted);
        md = format!(
            "{md}\n\n## Design Tokens\n\n```json\n{}\n```",
            super::json_str(&by_cat)
        );
    }

    if include_screenshots {
        let ids: Vec<String> = doc
            .get("children")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.get("id").and_then(|v| v.as_str()))
                    .filter(|s| !s.trim().is_empty())
                    .take(8)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        if !ids.is_empty() {
            let joined = ids.join(",");
            let image_data = files_api::get_images(
                &token, &file_key, &joined, None, Some("2"), Some("png"), None, None, None, None,
                None, None,
            )
            .await?;
            if let Some(images) = image_data.get("images").and_then(|v| v.as_object()) {
                let mut screenshots = String::from("\n\n## Screenshots\n\n");
                let lines: Vec<String> = images
                    .iter()
                    .map(|(id, url)| format!("- [{id}]({})", url.as_str().unwrap_or("")))
                    .collect();
                screenshots.push_str(&lines.join("\n"));
                md = format!("{md}{screenshots}");
            }
        }
    }

    write_output(&md, flags.get("output"))
}

pub async fn cmd_file_filter(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--type", "--name-regex", "--min-size"]);
    let visible_only = flag_present(args, "--visible-only");
    let has_fill = flag_present(args, "--has-fill");
    let no_children = flag_present(args, "--no-children");
    let input: Value = serde_json::from_str(&read_stdin())?;
    let opts = tree_filter::FilterOpts {
        types: tree_filter::parse_types(flags.get("type").unwrap_or("")),
        name_regex: flags.get("name_regex").map(String::from),
        visible_only,
        min_size: flags.get("min_size").and_then(tree_filter::parse_size),
        has_fill,
        no_children,
    };
    let filtered = tree_filter::filter_tree(&input, &opts).unwrap_or(Value::Null);
    print_data(&filtered, None)
}

pub async fn cmd_file_diff(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--from", "--to", "--depth", "--output"]);
    let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
    let depth = optional_int(flags.get("depth"));
    let from = require_value(
        flags.get("from").or_else(|| flags.arg(1)),
        "--from",
    );
    let to = require_value(flags.get("to").or_else(|| flags.arg(2)), "--to");

    let depth_str = depth.map(int_str);
    let (old_data, new_data) = tokio::try_join!(
        files_api::get_file(&token, &file_key, Some(&from), None, depth_str.as_deref(), None, None, None),
        files_api::get_file(&token, &file_key, Some(&to), None, depth_str.as_deref(), None, None, None),
    )?;
    let old_doc = old_data.get("document").cloned().unwrap_or(Value::Null);
    let new_doc = new_data.get("document").cloned().unwrap_or(Value::Null);
    let diff = crate::transform::diff::diff_trees(&old_doc, &new_doc);
    print_data(&diff, flags.get("output"))
}

pub async fn cmd_file_schema(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--component", "--depth", "--format", "--output"]);
    let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
    let component_id = require_value(
        flags.get("component").or_else(|| flags.arg(1)),
        "--component",
    );
    let depth = optional_int(flags.get("depth"));
    let format = flags.get("format").unwrap_or("json");
    let data = files_api::get_file(
        &token,
        &file_key,
        None,
        None,
        depth.map(int_str).as_deref(),
        None,
        None,
        None,
    )
    .await?;
    let doc = data.get("document").cloned().unwrap_or(Value::Null);
    match schema::infer_component_schema(&doc, &component_id) {
        Some(inferred) => {
            if format == "ts" {
                write_output(&schema::schema_to_typescript(&inferred), flags.get("output"))
            } else {
                print_data(&inferred, flags.get("output"))
            }
        }
        None => {
            eprintln!("Error: component not found");
            std::process::exit(1);
        }
    }
}

// --- Token extraction ---

pub async fn cmd_tokens_extract(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--depth", "--format", "--prefix", "--output", "--category"]);
    let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
    let depth = optional_int(flags.get("depth")).or_else(|| optional_int(flags.arg(1)));
    let format = flags.get("format").unwrap_or("json");
    let prefix = flags.get("prefix").unwrap_or("--figma-");
    let category = flags.get("category");
    let effective_depth = depth.unwrap_or(2);

    let data = files_api::get_file(
        &token,
        &file_key,
        None,
        None,
        Some(&int_str(effective_depth)),
        None,
        None,
        None,
    )
    .await?;
    let doc = data.get("document").cloned().unwrap_or(Value::Null);
    let simplified = compact::simplify_tree(&doc, depth);
    let extracted = tokens::extract_tokens(&simplified);
    let selected: Vec<Value> = match category {
        Some(cat) if cat != "all" => extracted
            .into_iter()
            .filter(|t| t.get("type").and_then(|v| v.as_str()) == Some(cat))
            .collect(),
        _ => extracted,
    };

    match tokens::format_tokens(&selected, format, prefix) {
        tokens::Formatted::Text(s) => write_output(&s, flags.get("output")),
        tokens::Formatted::Json(v) => {
            // For "json" format group by category; otherwise emit the value.
            if format == "json" {
                print_data(&tokens::tokens_by_category(&selected), flags.get("output"))
            } else {
                print_data(&v, flags.get("output"))
            }
        }
    }
}

// --- Image commands ---

pub async fn cmd_images_render(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let ids = require_arg(args, 1, "node-ids").to_string();
    let rest: Vec<String> = args.iter().skip(2).cloned().collect();
    let flags = parse_flags(&rest, &["--format", "--scale"]);
    let format = flags.get("format").unwrap_or("png").to_string();
    let scale = optional_float(flags.get("scale")).unwrap_or(2.0);
    let data = files_api::get_images(
        &token,
        &file_key,
        &ids,
        None,
        Some(&num_str(scale)),
        Some(&format),
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await?;
    print_data(&data, None)
}

pub async fn cmd_images_fills(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let data = files_api::get_image_fills(&token, &file_key).await?;
    print_data(&data, None)
}

pub async fn cmd_images_export(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let ids = require_arg(args, 1, "node-ids").to_string();
    let rest: Vec<String> = args.iter().skip(2).cloned().collect();
    let flags = parse_flags(&rest, &["--format", "--scale", "--dir", "--prefix"]);
    let format = flags.get("format").unwrap_or("png").to_string();
    let scale = optional_float(flags.get("scale")).unwrap_or(2.0);
    let manifest = flag_present(args, "--manifest");
    let node_ids: Vec<String> = ids.split(',').map(String::from).collect();

    let results = img_export::export_images(
        &token,
        &file_key,
        &node_ids,
        &format,
        &num_str(scale),
        flags.get("dir"),
        manifest,
        flags.get("prefix"),
    )
    .await?;

    println!("Exported images:");
    for (id, path) in &results {
        println!("  {id} -> {path}");
    }
    Ok(())
}

pub async fn cmd_image_export(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--ids"]);
    let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
    let (ids, remaining): (String, Vec<String>) = match flags.get("ids") {
        Some(ids) => (ids.to_string(), flags.rest.iter().skip(1).cloned().collect()),
        None => {
            let ids = require_arg(&flags.rest, 1, "node-ids").to_string();
            (ids, flags.rest.iter().skip(2).cloned().collect())
        }
    };
    let mut new_args = vec![file_key, ids];
    new_args.extend(remaining);
    cmd_images_export(&new_args).await
}

pub async fn cmd_assets_download(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let flags = parse_flags(&rest, &["--dir", "--prefix"]);
    let manifest = flag_present(args, "--manifest");

    let results = img_export::download_image_fills(
        &token,
        &file_key,
        flags.get("dir"),
        manifest,
        flags.get("prefix"),
    )
    .await?;

    println!("Downloaded assets:");
    for (id, path) in &results {
        println!("  {id} -> {path}");
    }
    Ok(())
}

// --- Comment commands ---

pub async fn cmd_comments_list(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let flags = parse_flags(&rest, &["--as-md"]);
    let as_md = flag_present(args, "--as-md") || flags.get("as_md") == Some("true");
    let as_md_str = if as_md { Some("true") } else { None };
    let data = comments_api::get_comments(&token, &file_key, as_md_str).await?;
    print_data(&data, None)
}

pub async fn cmd_comments_post(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let message = require_arg(args, 1, "message").to_string();
    let rest: Vec<String> = args.iter().skip(2).cloned().collect();
    let flags = parse_flags(&rest, &["--reply-to"]);
    let data = comments_api::post_comment(
        &token,
        &file_key,
        Some(&message),
        flags.get("reply_to"),
        None,
    )
    .await?;
    print_data(&data, None)
}

pub async fn cmd_comments_delete(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let comment_id = require_arg(args, 1, "comment-id").to_string();
    let data = comments_api::delete_comment(&token, &file_key, &comment_id).await?;
    print_data(&data, None)
}

// --- Project commands ---

pub async fn cmd_projects_list(args: &[String]) -> Result<()> {
    let token = require_token();
    let team_id = require_arg(args, 0, "team-id").to_string();
    let data = projects_api::get_team_projects(&token, &team_id).await?;
    print_data(&data, None)
}

pub async fn cmd_project_files(args: &[String]) -> Result<()> {
    let token = require_token();
    let project_id = require_arg(args, 0, "project-id").to_string();
    let data = projects_api::get_project_files(&token, &project_id, None).await?;
    print_data(&data, None)
}

// --- User commands ---

pub async fn cmd_me(_args: &[String]) -> Result<()> {
    let token = require_token();
    let data = users_api::get_me(&token).await?;
    print_data(&data, None)
}

// --- Component commands ---

pub async fn cmd_components_team(args: &[String]) -> Result<()> {
    let token = require_token();
    let team_id = require_arg(args, 0, "team-id").to_string();
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let flags = parse_flags(&rest, &["--page-size"]);
    let data = components_api::get_team_components(&token, &team_id, flags.get("page_size"), None, None).await?;
    print_data(&data, None)
}

pub async fn cmd_components_file(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let data = components_api::get_file_components(&token, &file_key).await?;
    print_data(&data, None)
}

pub async fn cmd_component_get(args: &[String]) -> Result<()> {
    let token = require_token();
    let component_key = require_arg(args, 0, "component-key").to_string();
    let data = components_api::get_component(&token, &component_key).await?;
    print_data(&data, None)
}

pub async fn cmd_component_sets_file(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let data = components_api::get_file_component_sets(&token, &file_key).await?;
    print_data(&data, None)
}

pub async fn cmd_components_list(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--team", "--page-size"]);
    if let Some(team) = flags.get("team") {
        let data = components_api::get_team_components(&token, team, flags.get("page_size"), None, None).await?;
        print_data(&data, None)
    } else {
        let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
        let data = components_api::get_file_components(&token, &file_key).await?;
        print_data(&data, None)
    }
}

// --- Style commands ---

pub async fn cmd_styles_team(args: &[String]) -> Result<()> {
    let token = require_token();
    let team_id = require_arg(args, 0, "team-id").to_string();
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let flags = parse_flags(&rest, &["--page-size"]);
    let data = styles_api::get_team_styles(&token, &team_id, flags.get("page_size"), None, None).await?;
    print_data(&data, None)
}

pub async fn cmd_styles_file(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let data = styles_api::get_file_styles(&token, &file_key).await?;
    print_data(&data, None)
}

pub async fn cmd_style_get(args: &[String]) -> Result<()> {
    let token = require_token();
    let style_key = require_arg(args, 0, "style-key").to_string();
    let data = styles_api::get_style(&token, &style_key).await?;
    print_data(&data, None)
}

pub async fn cmd_styles_list(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--team", "--page-size"]);
    if let Some(team) = flags.get("team") {
        let data = styles_api::get_team_styles(&token, team, flags.get("page_size"), None, None).await?;
        print_data(&data, None)
    } else {
        let file_key = require_arg(&flags.rest, 0, "file-key").to_string();
        let data = styles_api::get_file_styles(&token, &file_key).await?;
        print_data(&data, None)
    }
}

// --- Variable commands ---

pub async fn cmd_variables_local(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let data = variables_api::get_local_variables(&token, &file_key).await?;
    print_data(&data, None)
}

pub async fn cmd_variables_published(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let data = variables_api::get_published_variables(&token, &file_key).await?;
    print_data(&data, None)
}

// --- Dev resource commands ---

pub async fn cmd_dev_resources_list(args: &[String]) -> Result<()> {
    let token = require_token();
    let file_key = require_arg(args, 0, "file-key").to_string();
    let data = dev_resources_api::get_dev_resources(&token, &file_key, None).await?;
    print_data(&data, None)
}

// --- Webhook commands ---

pub async fn cmd_webhooks_list(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--context"]);
    let data = webhooks_api::get_webhooks(&token, flags.get("context"), None, None).await?;
    print_data(&data, None)
}

pub async fn cmd_webhooks_create(args: &[String]) -> Result<()> {
    let token = require_token();
    let event_type = require_arg(args, 0, "event-type").to_string();
    let team_id = require_arg(args, 1, "team-id").to_string();
    let endpoint = require_arg(args, 2, "endpoint").to_string();
    let body = json!({
        "event_type": event_type,
        "team_id": team_id,
        "endpoint": endpoint,
        "passcode": Value::Null,
        "description": Value::Null,
        "status": Value::Null,
    });
    let data = webhooks_api::create_webhook(&token, &body).await?;
    print_data(&data, None)
}

pub async fn cmd_webhooks_delete(args: &[String]) -> Result<()> {
    let token = require_token();
    let webhook_id = require_arg(args, 0, "webhook-id").to_string();
    let data = webhooks_api::delete_webhook(&token, &webhook_id).await?;
    print_data(&data, None)
}

// --- Discovery-adjacent: url parse, coverage, figma api ---

pub fn cmd_url_parse(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--output"]);
    let input = require_arg(&flags.rest, 0, "figma-url").to_string();
    let parsed = figma_url::parse_figma_url(&input);
    print_data(&parsed.to_json(), flags.get("output"))
}

pub fn cmd_figma_api_coverage(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--format", "--output"]);
    let report = api_coverage::coverage_report();
    if flags.get("format") == Some("md") {
        write_output(&api_coverage::coverage_report_markdown(&report), flags.get("output"))
    } else {
        print_data(&report, flags.get("output"))
    }
}

pub async fn cmd_figma_api_call(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--params", "--body", "--body-file", "--output"]);
    let operation_id = require_arg(&flags.rest, 0, "operation-id").to_string();
    let params = parse_json_map(flags.get("params"), "--params");
    let body = if flags.get("body").is_some() {
        parse_json_map(flags.get("body"), "--body")
    } else if let Some(bf) = flags.get("body_file") {
        let content = std::fs::read_to_string(bf)?;
        parse_json_map(Some(&content), "--body-file")
    } else {
        Value::Object(Default::default())
    };
    let explain = flag_present(args, "--explain-for-ai");

    if api_operations::write_operation(&operation_id) && !flag_present(args, "--yes") {
        eprintln!("Error: write operation requires --yes. Figma write APIs can mutate comments, variables, webhooks, or dev resources.");
        std::process::exit(1);
    }

    let data = api_operations::call_operation(&token, &operation_id, &params, &body).await?;
    if explain {
        print_data(
            &api_operations::result_envelope(&operation_id, data, None),
            flags.get("output"),
        )
    } else {
        print_data(&data, flags.get("output"))
    }
}

/// Format a float without a trailing `.0` for integers.
fn num_str(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

// --- Discovery / product / experience commands (M4) ---

pub fn cmd_discover(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--format", "--output"]);
    let manifest = crate::discovery::manifest();
    if flags.get("format") == Some("md") {
        write_output(&crate::discovery::manifest_markdown(&manifest), flags.get("output"))
    } else {
        print_data(&manifest, flags.get("output"))
    }
}

pub fn cmd_quickstart(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--format", "--output", "--figma-url"]);
    let figma_url = flags
        .get("figma_url")
        .map(String::from)
        .or_else(|| flags.arg(0).map(String::from));
    let report = crate::discovery::quickstart(figma_url.as_deref());
    if flags.get("format") == Some("json") {
        print_data(&report, flags.get("output"))
    } else {
        write_output(&crate::discovery::quickstart_markdown(&report), flags.get("output"))
    }
}

pub fn cmd_doctor(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--output"]);
    print_data(&crate::discovery::doctor(), flags.get("output"))
}

pub fn cmd_mcp_config(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--client", "--transport", "--port", "--command", "--output"]);
    let port = optional_int(flags.get("port")).unwrap_or(9449);
    let config = crate::discovery::mcp_config(
        flags.get("client").unwrap_or("generic"),
        flags.get("transport").unwrap_or("http"),
        port,
        flags.get("command").unwrap_or("fighorse"),
    );
    print_data(&config, flags.get("output"))
}

pub fn cmd_visual_audit(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--screenshot", "--platform", "--asset-format", "--notes", "--output"]);
    let figma_url = require_arg(&flags.rest, 0, "figma-url").to_string();
    let audit = crate::product::visual_audit::audit(
        Some(&figma_url),
        flags.get("screenshot"),
        flags.get("platform"),
        flags.get("asset_format"),
        flags.get("notes"),
    );
    print_data(&audit, flags.get("output"))
}

pub fn cmd_project_playbook(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--platform", "--asset-format", "--project-dir", "--output"]);
    let playbook = crate::product::playbook::build(
        flags.get("platform"),
        flags.get("asset_format"),
        flags.get("project_dir"),
    );
    print_data(&playbook, flags.get("output"))
}

pub async fn cmd_design_package(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(
        args,
        &[
            "--node-id", "--depth", "--max-tokens", "--output", "--screenshot-format",
            "--scale", "--screenshot-limit", "--platform", "--asset-format",
        ],
    );
    let input = require_arg(&flags.rest, 0, "figma-url-or-file-key").to_string();
    let opts = crate::product::design_package::PackageOpts {
        figma_url: Some(&input),
        file_key: None,
        node_id: flags.get("node_id"),
        depth: optional_int(flags.get("depth")).unwrap_or(2),
        max_tokens: optional_int(flags.get("max_tokens")).unwrap_or(8000),
        include_screenshot: !flag_present(args, "--no-screenshot"),
        include_assets: flag_present(args, "--include-assets"),
        screenshot_format: flags.get("screenshot_format").unwrap_or("png").to_string(),
        scale: optional_float(flags.get("scale")).unwrap_or(2.0),
        screenshot_limit: optional_int(flags.get("screenshot_limit")).unwrap_or(4) as usize,
        platform: flags.get("platform"),
        asset_format: flags.get("asset_format"),
    };
    let output = flags.get("output").map(String::from);
    let data = crate::product::design_package::get_design_package(&token, opts).await?;
    print_data(&data, output.as_deref())
}

pub async fn cmd_smoke(args: &[String]) -> Result<()> {
    let token = require_token();
    let flags = parse_flags(args, &["--output"]);
    let input = require_arg(&flags.rest, 0, "figma-url-or-file-key").to_string();
    let parsed = figma_url::parse_figma_url(&input);

    let opts = crate::product::design_package::PackageOpts {
        figma_url: Some(&input),
        file_key: None,
        node_id: None,
        depth: 1,
        max_tokens: 3000,
        include_screenshot: true,
        include_assets: false,
        screenshot_format: "png".to_string(),
        scale: 2.0,
        screenshot_limit: 4,
        platform: None,
        asset_format: None,
    };

    match crate::product::design_package::get_design_package(&token, opts).await {
        Ok(pkg) => {
            let status_ready = pkg
                .get("diagnostics")
                .and_then(|d| d.get("status"))
                .and_then(|v| v.as_str())
                == Some("ready");
            let mut next_steps = vec![Value::String(
                "Use fighorse design package with explicit --platform and --asset-format for implementation context.".into(),
            )];
            if parsed.node_id.is_none() {
                next_steps.push(Value::String("Copy a link to a selected frame, component, or group so the URL includes node-id.".into()));
            }
            if pkg.get("target").and_then(|t| t.get("type")).and_then(|v| v.as_str()) == Some("CANVAS") {
                next_steps.push(Value::String("Current target is a CANVAS/page; use screen_candidates from a design package to pick exact frames.".into()));
            }
            let out = json!({
                "kind": "fighorse.smoke.v1",
                "ok": status_ready,
                "parsed_input": parsed.to_json(),
                "source": pkg.get("source").cloned().unwrap_or(Value::Null),
                "file": pkg.get("file").cloned().unwrap_or(Value::Null),
                "target": pkg.get("target").cloned().unwrap_or(Value::Null),
                "diagnostics": pkg.get("diagnostics").cloned().unwrap_or(Value::Null),
                "next_steps": next_steps,
            });
            print_data(&out, flags.get("output"))
        }
        Err(e) => {
            let out = json!({
                "kind": "fighorse.smoke.v1",
                "ok": false,
                "error": err_message(&e),
                "parsed_input": parsed.to_json(),
                "checks": [
                    {"id": "token", "next_step": "Run fighorse auth login --token <FIGMA_TOKEN> or set FIGMA_TOKEN."},
                    {"id": "figma_url", "next_step": "Use fighorse quickstart \"<figma-frame-url>\" to verify URL parsing before smoke."},
                    {"id": "proxy", "next_step": "If your network requires a proxy, set HTTPS_PROXY or ALL_PROXY."}
                ],
                "next_step": "Run fighorse doctor --format json and verify FIGMA_TOKEN, file permissions, proxy, and Figma URL."
            });
            print_data(&out, flags.get("output"))?;
            std::process::exit(1);
        }
    }
}

// --- Experience commands ---

use crate::experience::{self as exp, Filters, ScopeOpts};

pub fn cmd_experience_schema(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--output"]);
    print_data(&exp::schema(), flags.get("output"))
}

pub fn cmd_experience_path(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--scope", "--project-dir", "--output"]);
    let opts = ScopeOpts {
        scope: flags.get("scope").map(String::from),
        project_dir: flags.get("project_dir").map(String::from),
    };
    let out = json!({
        "kind": "fighorse.experience-store.v1",
        "path": exp::experience_path(&opts).to_string_lossy(),
        "store": exp::store_info(&opts),
        "schema_version": exp::SCHEMA_VERSION,
        "records": exp::read_all(&opts).len(),
    });
    print_data(&out, flags.get("output"))
}

fn compact_map(pairs: Vec<(&str, Option<String>)>) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        if let Some(val) = v {
            if !val.trim().is_empty() {
                m.insert(k.to_string(), Value::String(val));
            }
        }
    }
    m
}

pub fn cmd_experience_add(args: &[String]) -> Result<()> {
    let flags = parse_flags(
        args,
        &[
            "--summary", "--lesson", "--category", "--severity", "--platform",
            "--asset-format", "--figma-url", "--file-key", "--node-id", "--tags",
            "--evidence", "--recommendation", "--client", "--command", "--json",
            "--scope", "--project-dir", "--output",
        ],
    );
    // stdin fallback when no --json and no --summary and stdin is piped.
    let stdin_json = if flags.get("json").is_none() && flags.get("summary").is_none() && !atty_stdin() {
        Some(read_stdin())
    } else {
        None
    };
    let base = parse_json_map(
        flags.get("json").or(stdin_json.as_deref()),
        "experience",
    );

    let extra = compact_map(vec![
        ("summary", flags.get("summary").map(String::from)),
        ("lesson", flags.get("lesson").map(String::from)),
        ("category", flags.get("category").map(String::from)),
        ("severity", flags.get("severity").map(String::from)),
        ("platform", flags.get("platform").map(String::from)),
        ("asset_format", flags.get("asset_format").map(String::from)),
        ("figma_url", flags.get("figma_url").map(String::from)),
        ("file_key", flags.get("file_key").map(String::from)),
        ("node_id", flags.get("node_id").map(String::from)),
        ("tags", flags.get("tags").map(String::from)),
        ("evidence", flags.get("evidence").map(String::from)),
        ("recommendation", flags.get("recommendation").map(String::from)),
        ("client", flags.get("client").map(String::from)),
        ("command", flags.get("command").map(String::from)),
    ]);

    let mut record = base.as_object().cloned().unwrap_or_default();
    for (k, v) in extra {
        record.insert(k, v);
    }

    let opts = ScopeOpts {
        scope: flags.get("scope").map(String::from),
        project_dir: flags.get("project_dir").map(String::from),
    };
    match exp::add(&Value::Object(record), &opts) {
        Ok(result) => print_data(&result, flags.get("output")),
        Err(e) => {
            println!("Error: {}", err_message(&e));
            std::process::exit(1);
        }
    }
}

pub fn cmd_experience_list(args: &[String]) -> Result<()> {
    let flags = parse_flags(
        args,
        &["--platform", "--asset-format", "--category", "--tag", "--limit", "--scope", "--project-dir", "--output"],
    );
    let limit = optional_int(flags.get("limit")).unwrap_or(8) as usize;
    let filters = Filters {
        platform: flags.get("platform").map(String::from),
        asset_format: flags.get("asset_format").map(String::from),
        category: flags.get("category").map(String::from),
        tag: flags.get("tag").map(String::from),
    };
    let opts = ScopeOpts {
        scope: flags.get("scope").map(String::from),
        project_dir: flags.get("project_dir").map(String::from),
    };
    print_data(&exp::list_experiences(&filters, limit, &opts), flags.get("output"))
}

pub fn cmd_experience_summary(args: &[String]) -> Result<()> {
    let flags = parse_flags(
        args,
        &["--platform", "--asset-format", "--category", "--tag", "--limit", "--format", "--scope", "--project-dir", "--output"],
    );
    let limit = optional_int(flags.get("limit")).unwrap_or(6) as usize;
    let filters = Filters {
        platform: flags.get("platform").map(String::from),
        asset_format: flags.get("asset_format").map(String::from),
        category: flags.get("category").map(String::from),
        tag: flags.get("tag").map(String::from),
    };
    let opts = ScopeOpts {
        scope: flags.get("scope").map(String::from),
        project_dir: flags.get("project_dir").map(String::from),
    };
    let data = exp::guidance(&filters, limit, &opts);
    if flags.get("format") == Some("md") {
        write_output(&exp::guidance_markdown(&data), flags.get("output"))
    } else {
        print_data(&data, flags.get("output"))
    }
}

/// Helper used by main to print an error and exit 1.
pub fn fail(e: &crate::error::Error) -> ! {
    eprintln!("Error: {}", err_message(e));
    std::process::exit(1);
}

// --- MCP ---

pub async fn cmd_mcp_serve(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--transport", "--port", "--host", "--cors-origin"]);
    let transport = flags.get("transport").unwrap_or("sse").to_string();
    let port = optional_int(flags.get("port")).unwrap_or(9449);
    crate::mcp::server::serve(&transport, port, flags.get("host"), flags.get("cors_origin")).await
}

// --- Install commands ---

use crate::install;

fn stdin_token_if_piped() -> Option<String> {
    if atty_stdin() {
        None
    } else {
        Some(read_stdin())
    }
}

pub fn cmd_install_home(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--home", "--output"]);
    print_data(&install::install_home(flags.get("home"))?, flags.get("output"))
}

pub fn cmd_install_auth(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--token", "--home", "--output"]);
    let apply = flag_present(args, "--apply");
    // clean_args = rest minus --apply.
    let clean: Vec<&String> = flags.rest.iter().filter(|a| a.as_str() != "--apply").collect();
    let stdin_tok = if apply { stdin_token_if_piped() } else { None };
    let token = flags
        .get("token")
        .map(String::from)
        .or_else(|| clean.first().map(|s| s.to_string()))
        .or_else(|| std::env::var("FIGMA_TOKEN").ok().filter(|s| !s.is_empty()))
        .or_else(|| std::env::var("FIGMA_API_KEY").ok().filter(|s| !s.is_empty()))
        .or(stdin_tok);
    print_data(
        &install::install_auth(token.as_deref(), flags.get("home"), apply)?,
        flags.get("output"),
    )
}

pub fn cmd_install_project(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--project-dir", "--output"]);
    print_data(&install::install_project(flags.get("project_dir"))?, flags.get("output"))
}

pub fn cmd_install_skill(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--dir", "--home", "--client", "--clients", "--output"]);
    let apply = flag_present(args, "--apply");
    print_data(
        &install::install_skill(flags.get("dir"), flags.get("home"), flags.get("client"), flags.get("clients"), apply)?,
        flags.get("output"),
    )
}

pub fn cmd_install_client(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--client", "--dir", "--transport", "--port", "--command", "--home", "--output"]);
    let apply = flag_present(args, "--apply");
    let port = optional_int(flags.get("port")).unwrap_or(9449);
    print_data(
        &install::install_client(
            flags.get("client"),
            flags.get("dir"),
            flags.get("transport").unwrap_or("http"),
            port,
            flags.get("command").unwrap_or("fighorse"),
            flags.get("home"),
            apply,
        )?,
        flags.get("output"),
    )
}

pub fn cmd_install_service(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--service", "--port", "--command", "--home", "--output"]);
    let apply = flag_present(args, "--apply");
    let port = optional_int(flags.get("port")).unwrap_or(9449);
    print_data(
        &install::install_service(
            flags.get("service").unwrap_or("auto"),
            port,
            flags.get("command").unwrap_or("fighorse"),
            flags.get("home"),
            apply,
        )?,
        flags.get("output"),
    )
}

pub fn cmd_install_binary(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--source", "--target", "--link-dir", "--link-dirs", "--home", "--output"]);
    let apply = flag_present(args, "--apply");
    print_data(
        &install::install_binary(
            flags.get("source"),
            flags.get("target"),
            flags.get("link_dir"),
            flags.get("link_dirs"),
            flags.get("home"),
            apply,
        )?,
        flags.get("output"),
    )
}

pub fn cmd_install_status(args: &[String]) -> Result<()> {
    let flags = parse_flags(args, &["--output"]);
    print_data(&install::status(), flags.get("output"))
}

fn install_opts_from<'a>(flags: &'a super::args::Flags, args: &'a [String]) -> install::InstallOpts<'a> {
    install::InstallOpts {
        source: flags.get("source"),
        path: flags.get("path").or_else(|| flags.get("project_dir")),
        target: flags.get("target"),
        default: flag_present(args, "--default"),
        client: flags.get("client"),
        clients: flags.get("clients"),
        transport: flags.get("transport").unwrap_or("http"),
        port: optional_int(flags.get("port")).unwrap_or(9449),
        command: flags.get("command").unwrap_or("fighorse"),
        home: flags.get("home"),
        token: flags.get("token"),
        mode: flags.get("mode"),
        service: flags.get("service").unwrap_or("auto"),
        link_dir: flags.get("link_dir"),
        link_dirs: flags.get("link_dirs"),
        no_service: flag_present(args, "--no-service"),
        apply: flag_present(args, "--apply"),
    }
}

pub fn cmd_install_self(args: &[String]) -> Result<()> {
    let flags = parse_flags(
        args,
        &["--source", "--path", "--target", "--client", "--clients", "--transport", "--port",
          "--command", "--home", "--token", "--mode", "--service", "--link-dir", "--link-dirs", "--output"],
    );
    let opts = install_opts_from(&flags, args);
    print_data(&install::install_self(&opts)?, flags.get("output"))
}

pub fn cmd_install_all(args: &[String]) -> Result<()> {
    let flags = parse_flags(
        args,
        &["--client", "--clients", "--transport", "--port", "--command", "--home", "--project-dir",
          "--source", "--target", "--link-dir", "--link-dirs", "--service", "--token", "--mode", "--output"],
    );
    let opts = install_opts_from(&flags, args);
    print_data(&install::install_all(&opts)?, flags.get("output"))
}
