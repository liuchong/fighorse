//! fighorse CLI entry point.
//!
//! Routes commands to API modules and AI enhancements.

use fighorse::cli::commands;
use fighorse::config;
use fighorse::error::Result;

use std::process::ExitCode;

/// Apply proxy settings so downstream HTTP clients pick them up, mirroring
/// `config/setup-proxy!` which echoes the chosen proxy to stderr.
fn setup_proxy(proxy: &Option<String>) {
    if let Some(proxy_url) = proxy {
        if std::env::var("HTTP_PROXY")
            .map(|v| v.is_empty())
            .unwrap_or(true)
        {
            std::env::set_var("HTTP_PROXY", proxy_url);
        }
        if std::env::var("HTTPS_PROXY")
            .map(|v| v.is_empty())
            .unwrap_or(true)
        {
            std::env::set_var("HTTPS_PROXY", proxy_url);
        }
        eprintln!("Using proxy: {proxy_url}");
    }
}

fn main() -> ExitCode {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    runtime.block_on(async_main())
}

/// Turn a command result into an exit code, printing errors to stderr.
fn finish(result: Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn async_main() -> ExitCode {
    let cfg = config::load_config();
    setup_proxy(&cfg.proxy);

    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd1 = args.first().map(String::as_str);
    let cmd2 = args.get(1).map(String::as_str);

    let is_version = args.iter().any(|a| a == "--version" || a == "-V");
    if is_version {
        println!("fighorse {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let is_help = matches!(cmd1, Some("help")) || args.iter().any(|a| a == "--help" || a == "-h");
    if is_help {
        print_help();
        return ExitCode::SUCCESS;
    }

    // Args after the leading one/two command words.
    let rest1: Vec<String> = args.iter().skip(1).cloned().collect();
    let rest2: Vec<String> = args.iter().skip(2).cloned().collect();

    match (cmd1, cmd2) {
        // Discovery / coverage / figma api
        (Some("quickstart"), _) => finish(commands::cmd_quickstart(&rest1)),
        (Some("discover"), _) => finish(commands::cmd_discover(&rest1)),
        (Some("doctor"), _) => finish(commands::cmd_doctor(&rest1)),
        (Some("smoke"), _) => finish(commands::cmd_smoke(&rest1).await),
        (Some("url"), Some("parse")) => finish(commands::cmd_url_parse(&rest2)),
        (Some("design"), Some("package")) => finish(commands::cmd_design_package(&rest2).await),
        (Some("mcp"), Some("config")) => finish(commands::cmd_mcp_config(&rest2)),
        (Some("figma-api"), Some("coverage")) => finish(commands::cmd_figma_api_coverage(&rest2)),
        (Some("figma"), Some("api")) => finish(commands::cmd_figma_api_call(&rest2).await),
        (Some("code-connect"), Some("generate")) => {
            finish(commands::cmd_code_connect_generate(&rest2).await)
        }
        (Some("code-connect"), Some("parse")) => finish(commands::cmd_code_connect_parse(&rest2)),
        (Some("code-connect"), Some("validate")) => {
            finish(commands::cmd_code_connect_validate(&rest2).await)
        }
        (Some("code-connect"), Some("preview")) => {
            finish(commands::cmd_code_connect_preview(&rest2).await)
        }
        (Some("code-connect"), Some("publish")) => {
            finish(commands::cmd_code_connect_publish(&rest2).await)
        }
        (Some("code-connect"), Some("unpublish")) => {
            finish(commands::cmd_code_connect_unpublish(&rest2).await)
        }
        (Some("visual"), Some("audit")) => finish(commands::cmd_visual_audit(&rest2)),
        (Some("project"), Some("playbook")) => finish(commands::cmd_project_playbook(&rest2)),

        // Experience
        (Some("experience"), Some("schema")) => finish(commands::cmd_experience_schema(&rest2)),
        (Some("experience"), Some("summary")) => finish(commands::cmd_experience_summary(&rest2)),
        (Some("experience"), Some("list")) => finish(commands::cmd_experience_list(&rest2)),
        (Some("experience"), Some("add")) => finish(commands::cmd_experience_add(&rest2)),
        (Some("experience"), Some("path")) => finish(commands::cmd_experience_path(&rest2)),

        // Install: `install` with a --flag or nothing after → self; subcommands otherwise.
        (Some("install"), Some("self")) => finish(commands::cmd_install_self(&rest2).await),
        (Some("install"), Some("home")) => finish(commands::cmd_install_home(&rest2)),
        (Some("install"), Some("auth")) => finish(commands::cmd_install_auth(&rest2)),
        (Some("install"), Some("project")) => finish(commands::cmd_install_project(&rest2)),
        (Some("install"), Some("binary")) => finish(commands::cmd_install_binary(&rest2)),
        (Some("install"), Some("client")) => finish(commands::cmd_install_client(&rest2)),
        (Some("install"), Some("service")) => finish(commands::cmd_install_service(&rest2).await),
        (Some("install"), Some("skill")) => finish(commands::cmd_install_skill(&rest2)),
        (Some("install"), Some("all")) => finish(commands::cmd_install_all(&rest2).await),
        (Some("install"), Some("status")) => finish(commands::cmd_install_status(&rest2)),
        (Some("install"), Some("verify")) => finish(commands::cmd_install_verify(&rest2).await),
        (Some("install"), Some("rollback")) => finish(commands::cmd_install_rollback(&rest2)),
        (Some("install"), Some(sub)) if sub.starts_with("--") => {
            finish(commands::cmd_install_self(&rest1).await)
        }
        (Some("install"), None) => finish(commands::cmd_install_all(&rest1).await),

        // Auth
        (Some("auth"), Some("login")) => finish(commands::cmd_auth_login(&rest2)),
        (Some("auth"), Some("logout")) => finish(commands::cmd_auth_logout(&rest2)),
        (Some("auth"), Some("status")) => finish(commands::cmd_auth_status(&rest2)),

        // File commands
        (Some("file"), Some("get")) => finish(commands::cmd_file_get(&rest2).await),
        (Some("file"), Some("nodes")) => finish(commands::cmd_file_nodes(&rest2).await),
        (Some("file"), Some("meta")) => finish(commands::cmd_file_meta(&rest2).await),
        (Some("file"), Some("versions")) => finish(commands::cmd_file_versions(&rest2).await),
        (Some("file"), Some("compact")) => finish(commands::cmd_file_compact(&rest2).await),
        (Some("file"), Some("filter")) => finish(commands::cmd_file_filter(&rest2).await),
        (Some("file"), Some("diff")) => finish(commands::cmd_file_diff(&rest2).await),
        (Some("file"), Some("tree")) => finish(commands::cmd_file_tree(&rest2).await),
        (Some("file"), Some("to-md")) => finish(commands::cmd_file_to_md(&rest2).await),
        (Some("file"), Some("tokens")) => finish(commands::cmd_tokens_extract(&rest2).await),
        (Some("file"), Some("schema")) => finish(commands::cmd_file_schema(&rest2).await),
        (Some("compact"), _) => finish(commands::cmd_compact_stdin(&rest1)),

        // Node alias
        (Some("node"), Some("get")) => finish(commands::cmd_file_nodes(&rest2).await),

        // Image commands
        (Some("image"), Some("export")) => finish(commands::cmd_image_export(&rest2).await),
        (Some("images"), Some("render")) => finish(commands::cmd_images_render(&rest2).await),
        (Some("images"), Some("fills")) => finish(commands::cmd_images_fills(&rest2).await),
        (Some("images"), Some("export")) => finish(commands::cmd_images_export(&rest2).await),
        (Some("asset"), Some("download")) => finish(commands::cmd_assets_download(&rest2).await),

        // Comment commands
        (Some("comments"), Some("list")) => finish(commands::cmd_comments_list(&rest2).await),
        (Some("comments"), Some("post")) => finish(commands::cmd_comments_post(&rest2).await),
        (Some("comments"), Some("delete")) => finish(commands::cmd_comments_delete(&rest2).await),

        // Project commands
        (Some("projects"), Some("list")) => finish(commands::cmd_projects_list(&rest2).await),
        (Some("project"), Some("files")) => finish(commands::cmd_project_files(&rest2).await),

        // User commands
        (Some("me"), _) => finish(commands::cmd_me(&rest1).await),

        // Component commands
        (Some("components"), Some("list")) => finish(commands::cmd_components_list(&rest2).await),
        (Some("components"), Some("team")) => finish(commands::cmd_components_team(&rest2).await),
        (Some("components"), Some("file")) => finish(commands::cmd_components_file(&rest2).await),
        (Some("component"), Some("get")) => finish(commands::cmd_component_get(&rest2).await),
        (Some("component"), Some("export")) => finish(commands::cmd_image_export(&rest2).await),
        (Some("component-sets"), Some("file")) => {
            finish(commands::cmd_component_sets_file(&rest2).await)
        }

        // Style commands
        (Some("styles"), Some("list")) => finish(commands::cmd_styles_list(&rest2).await),
        (Some("styles"), Some("team")) => finish(commands::cmd_styles_team(&rest2).await),
        (Some("styles"), Some("file")) => finish(commands::cmd_styles_file(&rest2).await),
        (Some("style"), Some("get")) => finish(commands::cmd_style_get(&rest2).await),

        // Variable commands
        (Some("variables"), Some("list")) => finish(commands::cmd_variables_local(&rest2).await),
        (Some("variables"), Some("local")) => finish(commands::cmd_variables_local(&rest2).await),
        (Some("variables"), Some("published")) => {
            finish(commands::cmd_variables_published(&rest2).await)
        }

        // Dev resource commands
        (Some("dev-resources"), Some("list")) => {
            finish(commands::cmd_dev_resources_list(&rest2).await)
        }

        // Webhook commands
        (Some("webhooks"), Some("list")) => finish(commands::cmd_webhooks_list(&rest2).await),
        (Some("webhooks"), Some("create")) => finish(commands::cmd_webhooks_create(&rest2).await),
        (Some("webhooks"), Some("delete")) => finish(commands::cmd_webhooks_delete(&rest2).await),

        // Token extraction
        (Some("tokens"), Some("extract")) => finish(commands::cmd_tokens_extract(&rest2).await),

        // MCP
        (Some("mcp"), Some("serve")) => finish(commands::cmd_mcp_serve(&rest2).await),
        (Some("mcp"), None) => finish(commands::cmd_mcp_serve(&rest1).await),

        _ => {
            print_help();
            ExitCode::SUCCESS
        }
    }
}

fn print_help() {
    print!(
        r#"fighorse — Figma data Swiss Army knife, shaped for AI consumption

Usage: fighorse <command> [args...]

Start here:
  fighorse quickstart                         Guided first-run setup
  fighorse quickstart "<figma-frame-url>"      Validate token, frame link, and next command
  fighorse quickstart "<figma-frame-url>" --format json
                                                Machine-readable setup check for AI clients

Run quickstart first. It explains required Figma token setup, frame-link scope,
design-package commands, and MCP client setup when needed.

Self Discovery and AI Replication:
  quickstart [figma-url] [--format json]        Guided first-run readiness check
  discover [--format json|md]                  Describe capabilities for AI tools
  doctor [--format json]                       Check runtime/auth readiness
  smoke <figma-url>                            Verify real Figma access and design package readiness
  url parse <figma-url>                        Parse file_key and node_id
  design package <figma-url> [--platform P] [--asset-format F]  Build AI replication package
  mcp config [--client C] [--transport T]      Emit MCP client config
  figma-api coverage [--format json|md]        Report official Figma REST OpenAPI coverage
  figma api <operationId> --params JSON [--body JSON|--body-file P] [--yes]  Call any covered REST operation
  code-connect generate <figma-url> --context P [--output P]  Generate a modern parserless Code Connect template
  code-connect parse [--dir D|--file P]         Parse local .figma.ts/.figma.js documents without executing them
  code-connect validate [--documents P|--dir D] Validate Code Connect documents against Figma nodes
  code-connect preview [--documents P|--dir D]  Render snippets through Figma's observed preview protocol
  code-connect publish [--documents P|--dir D] --dry-run|--yes [--force]  Publish mappings to Figma Dev Mode
  code-connect unpublish (--node URL --label L|--dir D) --dry-run|--yes  Remove Code Connect mappings
  visual audit <figma-url> [--screenshot P]    Produce AI-ready visual fidelity audit guidance
  project playbook [--platform P]              Produce project-level fighorse AI playbook

Experience / Self Learning:
  experience schema                            Show versioned JSONL record schema
  experience summary [--platform P] [--asset-format F] [--scope global|project|merged]  Prompt-ready local lessons
  experience list [--platform P] [--category C] [--tag T]  List stored lessons
  experience add --summary S --lesson L [...]  Record a reusable Figma replication lesson
  experience path                              Show local experience store path

Install:
  install home [--home D]                       Create ~/.fighorse directories
  install auth [--token T] [--apply]            Persist local Figma token for CLI and MCP
  install binary --source P [--apply]           Install CLI binary into fighorse home and PATH links
  install project [--project-dir D]             Enable project-scoped .fighorse experience
  install client --client cursor|codex|kimi|claude|opencode|openclaw|hermes-agent [--apply]  Generate or apply client MCP setup
  install service [--service launchd|systemd] [--apply]  Generate or apply auto-start MCP HTTP service
  install skill [--dir D] [--clients C] [--apply]  Apply canonical skills/rule and safely migrate generated legacy copies
  install [--default|--path D|--target P] [--mode cli|service] [--apply]  Self-install this binary and emit AI-readable install guidance
  install self [--default|--path D|--target P] [--apply]  Same as install root command
  install all [--mode cli|service|all] [--no-service] [--clients C] [--source P] [--apply]  Generate or apply setup; default mode is cli
  install status                                Show install paths and detected state
  install verify [--home D] [--port N]          Verify manifest, binary, service handshake, clients, and skills
  install rollback [--home D]                   Restore unchanged managed files from manifest backups
  Service setup: fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply
  Claude only:  fighorse install client --client claude --apply
  Service transaction order: binary/service -> /health -> initialize + tools/list -> clients -> skills -> manifest verification
  Canonical skills: ~/.agents/skills/fighorse/SKILL.md (Cursor/Kimi/Codex), ~/.claude/skills/fighorse/SKILL.md, ~/.cursor/rules/fighorse.mdc

Auth:
  auth login --token <token>                    Save Figma token
  auth logout                                   Remove saved token
  auth status                                   Show auth status

File Operations:
  file get <file-key> [depth] [--version V] [--output P]  Fetch raw file JSON
  file nodes <file-key> <ids> [--depth N]       Fetch specific nodes
  node get <file-key> <id> [--depth N]          Fetch a specific node
  file meta <file-key>                          Get file metadata
  file versions <file-key> [--page-size N]      Get version history
  file compact <file-key> [depth] [--max-tokens N]  Smart truncation for AI
  compact < input.json [--max-tokens N]       Compact JSON from stdin
  file filter < input.json [--type T] [--visible-only]  Filter a tree
  file diff <file-key> --from V1 --to V2        Diff two versions
  file tree <file-key> [depth]                  Lightweight structure view
  file to-md <file-key> [depth] [--title T]     Export as Markdown
  file tokens <file-key> [--format json|css|scss|tailwind]  Extract tokens
  file schema <file-key> --component ID [--format json|ts]  Infer component props

Image Operations:
  Recommended export dirs: ./.fighorse/exports for scratch, ./assets/fighorse for packaged assets
  image export <file-key> --ids <ids> [--format F] [--scale S] [--dir D] [--manifest]  Download node slices
  images render <file-key> <ids> [--format F] [--scale S]  Get render URLs
  images fills <file-key>                        Get image fill URLs
  images export <file-key> <ids> [--format F] [--scale S] [--dir D] [--manifest]  Download node slices
  asset download <file-key> [--dir D] [--manifest]  Download all image fills

Comments:
  comments list <file-key> [--as-md]            List comments
  comments post <file-key> <message> [--reply-to ID]  Post comment
  comments delete <file-key> <comment-id>       Delete comment

Projects:
  projects list <team-id>                       List team projects
  project files <project-id>                    List project files

Users:
  me                                            Get current user

Components:
  components list <file-key>                    List file components
  components list --team <team-id> [--page-size N]  List team components
  components team <team-id> [--page-size N]     List team components
  components file <file-key>                    List file components
  component get <component-key>                 Get component metadata
  component export <file-key> --ids <ids> [--format F] [--dir D]  Export component/control nodes
  component-sets file <file-key>                List file component sets

Styles:
  styles list <file-key>                        List file styles
  styles list --team <team-id> [--page-size N]  List team styles
  styles team <team-id> [--page-size N]         List team styles
  styles file <file-key>                        List file styles
  style get <style-key>                         Get style metadata

Variables:
  variables list <file-key>                     Get local variables
  variables local <file-key>                    Get local variables
  variables published <file-key>                Get published variables

Dev Resources:
  dev-resources list <file-key>                 List dev resources

Webhooks:
  webhooks list [--context C]                   List webhooks
  webhooks create <event-type> <team-id> <endpoint>  Create webhook
  webhooks delete <webhook-id>                  Delete webhook

Design Tokens:
  tokens extract <file-key> [depth]             Extract design tokens

MCP Server:
  mcp serve [--transport http|stdio] [--port N] [--host 127.0.0.1]  Start MCP server
  HTTP endpoint: http://127.0.0.1:9449/mcp (official rmcp stateful sessions; JSON or event-stream responses)
  --transport sse fails with migration guidance; legacy /sse and /messages endpoints are not served

Environment:
  FIGMA_TOKEN    Figma Personal Access Token
  FIGHORSE_HOME  Default: ~/.fighorse
  FIGHORSE_MCP_MODE  MCP safety mode: readonly (default) or write
  FIGHORSE_MCP_LOCAL_WRITE  Set to allow for MCP local asset exports inside approved roots
  FIGHORSE_MCP_CODE_CONNECT  Set to allow before MCP preview/publish sends Code Connect templates to Figma
  FIGHORSE_MCP_ALLOW_MULTIPLE  Set to 1 only for development when bypassing the MCP singleton lock
  FIGHORSE_HTTP_TIMEOUT_MS  Figma REST request timeout, default 120000
  FIGHORSE_EXPERIENCE_PATH  Override local experience JSONL store
  FIGHORSE_EXPERIENCE_SCOPE  auto (default), global, project, or merged
  HTTP_PROXY     HTTP proxy URL (e.g. http://127.0.0.1:7897)
  HTTPS_PROXY    HTTPS proxy URL
  ALL_PROXY      Fallback proxy URL

Proxy Example:
  HTTPS_PROXY=http://127.0.0.1:7897 fighorse file meta <file-key>
"#
    );
}
