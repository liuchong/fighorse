//! Image export — render and download images from Figma files.
//!
//! Image export. Uses the Figma `/v1/images` API to get
//! URLs, then downloads them into an allow-listed export directory.

use crate::api::files;
use crate::error::{Error, Result};
use crate::http;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const DEFAULT_EXPORT_DIR: &str = "./.fighorse/exports";

/// Normalize a format string to one of svg/pdf/jpg/png.
pub fn normalize_format(format: &str) -> String {
    match format.to_lowercase().as_str() {
        "svg" => "svg",
        "pdf" => "pdf",
        "jpg" | "jpeg" => "jpg",
        _ => "png",
    }
    .to_string()
}

fn format_extension(format: &str) -> &'static str {
    match normalize_format(format).as_str() {
        "svg" => ".svg",
        "pdf" => ".pdf",
        "jpg" => ".jpg",
        _ => ".png",
    }
}

fn content_type_extension(content_type: &str) -> &'static str {
    let ct = content_type.to_lowercase();
    if ct.contains("png") {
        ".png"
    } else if ct.contains("jpeg") || ct.contains("jpg") {
        ".jpg"
    } else if ct.contains("svg") {
        ".svg"
    } else if ct.contains("webp") {
        ".webp"
    } else if ct.contains("gif") {
        ".gif"
    } else if ct.contains("pdf") {
        ".pdf"
    } else {
        ""
    }
}

fn has_extension(dest_path: &str) -> bool {
    regex::Regex::new(r"\.[A-Za-z0-9]+$")
        .unwrap()
        .is_match(dest_path)
}

fn ensure_extension(dest_path: &str, content_type: &str, fallback_ext: &str) -> String {
    if has_extension(dest_path) {
        return dest_path.to_string();
    }
    let detected = content_type_extension(content_type);
    if detected.is_empty() {
        format!("{dest_path}{fallback_ext}")
    } else {
        format!("{dest_path}{detected}")
    }
}

fn safe_name(s: &str) -> String {
    let non_allowed = regex::Regex::new(r"[^A-Za-z0-9._-]+").unwrap();
    let clean = non_allowed.replace_all(s, "_");
    let leading = regex::Regex::new(r"^_+").unwrap();
    let clean = leading.replace(&clean, "");
    if clean.trim().is_empty() {
        "asset".to_string()
    } else {
        clean.into_owned()
    }
}

fn allowed_export_roots() -> Vec<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        cwd.join(".fighorse").join("exports"),
        cwd.join("assets").join("fighorse"),
        home.join(".fighorse").join("exports"),
    ]
}

fn child_path(root: &Path, target: &Path) -> bool {
    root == target || target.starts_with(root)
}

/// Find the nearest existing ancestor directory of `target`.
fn existing_ancestor(target: &Path) -> PathBuf {
    let mut dir = target.to_path_buf();
    loop {
        if dir.exists() {
            return dir;
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent.to_path_buf(),
            _ => return dir,
        }
    }
}

/// Canonicalize a path, resolving symlinks up to the nearest existing ancestor.
fn canonical_path(target: &Path) -> PathBuf {
    let ancestor = existing_ancestor(target);
    let relative = target.strip_prefix(&ancestor).unwrap_or(target);
    let real_ancestor = std::fs::canonicalize(&ancestor).unwrap_or(ancestor);
    real_ancestor.join(relative)
}

fn first_allowed_root(dest_dir: &Path) -> Option<PathBuf> {
    let target = canonical_path(dest_dir);
    allowed_export_roots().into_iter().find(|root| {
        let canonical_root = canonical_path(root);
        child_path(&canonical_root, &target)
    })
}

/// Resolve and create the export directory, enforcing the allow-list.
fn safe_export_dir(dest_dir: Option<&str>) -> Result<PathBuf> {
    let dest = dest_dir.unwrap_or(DEFAULT_EXPORT_DIR);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let resolved = if Path::new(dest).is_absolute() {
        PathBuf::from(dest)
    } else {
        cwd.join(dest)
    };

    let root = first_allowed_root(&resolved).ok_or_else(|| {
        Error::Usage(format!(
            "Export directory is outside allowed roots: {dest}. Use ./.fighorse/exports, ./assets/fighorse, or ~/.fighorse/exports."
        ))
    })?;

    std::fs::create_dir_all(&resolved)?;
    let real_root = std::fs::canonicalize(&root).unwrap_or(root);
    let real_dest = std::fs::canonicalize(&resolved).unwrap_or(resolved.clone());
    if !child_path(&real_root, &real_dest) {
        return Err(Error::Usage(format!(
            "Export directory escapes allowed root: {dest}"
        )));
    }
    Ok(real_dest)
}

fn write_manifest(dir: &Path, kind: &str, entries: &[Value]) -> Result<()> {
    let manifest = json!({
        "kind": kind,
        "generated_by": "fighorse",
        "entries": entries,
    });
    let content = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(dir.join("manifest.json"), content)?;
    Ok(())
}

/// Download an image from a URL to `dest_path`, returning the written path.
async fn fetch_image(url: &str, dest_path: &str, fallback_ext: &str) -> Result<String> {
    let resp = http::raw_get(url).await?;
    if !resp.status().is_success() {
        return Err(Error::Other(format!(
            "Failed to download image: HTTP {}",
            resp.status().as_u16()
        )));
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let final_path = ensure_extension(dest_path, &content_type, fallback_ext);
    let bytes = resp.bytes().await.map_err(Error::from)?;
    if let Some(parent) = Path::new(&final_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&final_path, &bytes)?;
    Ok(final_path)
}

/// Result of an export: ordered (id, path) pairs.
pub struct ExportResult {
    pub rows: Vec<(String, String)>,
}

/// Render and download images for the given node ids.
pub async fn export_images(
    token: &str,
    file_key: &str,
    node_ids: &[String],
    format: &str,
    scale: &str,
    dest_dir: Option<&str>,
    manifest: bool,
    prefix: Option<&str>,
) -> Result<Vec<(String, String)>> {
    let format = normalize_format(format);
    let ids_joined = node_ids.join(",");
    let result = files::get_images(
        token,
        file_key,
        &ids_joined,
        None,
        Some(scale),
        Some(&format),
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await?;

    let images = result.get("images").cloned().unwrap_or(Value::Null);
    let dir = safe_export_dir(dest_dir)?;
    let ext = format_extension(&format);
    let filename_prefix = match prefix {
        Some(p) if !p.trim().is_empty() => safe_name(p),
        _ => String::new(),
    };

    let mut rows: Vec<(String, String)> = Vec::new();
    let mut entries: Vec<Value> = Vec::new();

    if let Some(obj) = images.as_object() {
        for (node_id, url_val) in obj {
            let url = match url_val.as_str() {
                Some(u) if !u.is_empty() => u,
                _ => continue,
            };
            let target = dir.join(format!("{filename_prefix}{}{ext}", safe_name(node_id)));
            let written = fetch_image(url, &target.to_string_lossy(), ext).await?;
            entries.push(json!({
                "node_id": node_id,
                "path": written,
                "format": format,
                "scale": scale,
                "source_url": url,
            }));
            rows.push((node_id.clone(), written));
        }
    }

    if manifest {
        write_manifest(&dir, "fighorse.image_export", &entries)?;
    }
    Ok(rows)
}

/// Download all image fills in a file.
pub async fn download_image_fills(
    token: &str,
    file_key: &str,
    dest_dir: Option<&str>,
    manifest: bool,
    prefix: Option<&str>,
) -> Result<Vec<(String, String)>> {
    let result = files::get_image_fills(token, file_key).await?;
    let images = result
        .get("meta")
        .and_then(|m| m.get("images"))
        .or_else(|| result.get("images"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));

    let dir = safe_export_dir(dest_dir)?;
    let filename_prefix = match prefix {
        Some(p) if !p.trim().is_empty() => safe_name(p),
        _ => String::new(),
    };

    let mut rows: Vec<(String, String)> = Vec::new();
    let mut entries: Vec<Value> = Vec::new();

    if let Some(obj) = images.as_object() {
        for (image_ref, url_val) in obj {
            let url = match url_val.as_str() {
                Some(u) if !u.is_empty() => u,
                _ => continue,
            };
            let target = dir.join(format!("{filename_prefix}{}", safe_name(image_ref)));
            let written = fetch_image(url, &target.to_string_lossy(), "").await?;
            entries.push(json!({
                "image_ref": image_ref,
                "path": written,
                "source_url": url,
            }));
            rows.push((image_ref.clone(), written));
        }
    }

    if manifest {
        write_manifest(&dir, "fighorse.asset_download", &entries)?;
    }
    Ok(rows)
}
