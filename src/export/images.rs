//! Image export — render and download images from Figma files.
//!
//! Image export. Uses the Figma `/v1/images` API to get
//! URLs, then downloads them into an allow-listed export directory.

use crate::api::files;
use crate::config;
use crate::error::{Error, Result};
use crate::http;
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_EXPORT_DIR: &str = "./.fighorse/exports";
static EXPORT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    let cwd = canonical_anchor(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let home = canonical_anchor(&dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    let mut roots = vec![
        cwd.join(".fighorse").join("exports"),
        cwd.join("assets").join("fighorse"),
        home.join(".fighorse").join("exports"),
    ];
    if std::env::var("FIGHORSE_MCP_SERVICE").is_ok_and(|value| !value.is_empty()) {
        roots.push(canonical_anchor(&config::fighorse_home()).join("exports"));
    }
    roots
}

fn child_path(root: &Path, target: &Path) -> bool {
    root == target || target.starts_with(root)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn canonical_anchor(path: &Path) -> PathBuf {
    canonical_path(path)
}

/// Find the nearest existing ancestor directory of `target`.
fn existing_ancestor(target: &Path) -> PathBuf {
    let mut dir = normalize_path(target);
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
    let normalized = normalize_path(target);
    let ancestor = existing_ancestor(&normalized);
    let relative = normalized.strip_prefix(&ancestor).unwrap_or(&normalized);
    let real_ancestor = std::fs::canonicalize(&ancestor).unwrap_or(ancestor);
    normalize_path(&real_ancestor.join(relative))
}

fn first_allowed_root(dest_dir: &Path) -> Option<PathBuf> {
    let target = canonical_path(dest_dir);
    allowed_export_roots().into_iter().find(|root| {
        let logical_root = normalize_path(root);
        let canonical_root = canonical_path(root);
        canonical_root == logical_root && child_path(&canonical_root, &target)
    })
}

/// Resolve and create the export directory, enforcing the allow-list.
fn safe_export_dir(dest_dir: Option<&str>) -> Result<PathBuf> {
    let service_default = std::env::var("FIGHORSE_MCP_SERVICE")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|_| config::fighorse_home().join("exports"));
    let dest_path = match dest_dir {
        Some(dest) => PathBuf::from(dest),
        None => service_default.unwrap_or_else(|| PathBuf::from(DEFAULT_EXPORT_DIR)),
    };
    let dest = dest_path.to_string_lossy();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let resolved = if dest_path.is_absolute() {
        normalize_path(&dest_path)
    } else {
        normalize_path(&cwd.join(&dest_path))
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

#[cfg(unix)]
fn atomic_replace_export_file(path: &Path, content: &[u8]) -> Result<()> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;

    let parent = path
        .parent()
        .ok_or_else(|| Error::Usage(format!("Export path has no parent: {}", path.display())))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| Error::Usage(format!("Export path has no file name: {}", path.display())))?;
    let parent_handle = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent)?;
    let dir_fd = parent_handle.as_raw_fd();

    let target_name = CString::new(file_name.as_bytes())
        .map_err(|_| Error::Usage("Export file name contains a NUL byte".into()))?;
    let temp_name = CString::new(format!(
        ".fighorse-write-{}-{}",
        std::process::id(),
        EXPORT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
    .expect("generated export temporary file name cannot contain NUL");
    let temp_fd = unsafe {
        libc::openat(
            dir_fd,
            temp_name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if temp_fd < 0 {
        return Err(Error::from(std::io::Error::last_os_error()));
    }

    let mut temp_file = unsafe { std::fs::File::from_raw_fd(temp_fd) };
    if let Err(error) = temp_file
        .write_all(content)
        .and_then(|_| temp_file.sync_all())
    {
        drop(temp_file);
        unsafe {
            libc::unlinkat(dir_fd, temp_name.as_ptr(), 0);
        }
        return Err(Error::from(error));
    }
    drop(temp_file);

    let renamed =
        unsafe { libc::renameat(dir_fd, temp_name.as_ptr(), dir_fd, target_name.as_ptr()) };
    if renamed != 0 {
        let error = std::io::Error::last_os_error();
        unsafe {
            libc::unlinkat(dir_fd, temp_name.as_ptr(), 0);
        }
        return Err(Error::from(error));
    }
    Ok(())
}

#[cfg(not(unix))]
fn atomic_replace_export_file(path: &Path, content: &[u8]) -> Result<()> {
    std::fs::write(path, content)?;
    Ok(())
}

fn write_manifest(dir: &Path, kind: &str, entries: &[Value]) -> Result<()> {
    let manifest = json!({
        "kind": kind,
        "generated_by": "fighorse",
        "entries": entries,
    });
    let content = serde_json::to_string_pretty(&manifest)?;
    atomic_replace_export_file(&dir.join("manifest.json"), content.as_bytes())?;
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
    atomic_replace_export_file(Path::new(&final_path), &bytes)?;
    Ok(final_path)
}

/// Result of an export: ordered (id, path) pairs.
pub struct ExportResult {
    pub rows: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy)]
pub struct ExportOptions<'a> {
    pub format: &'a str,
    pub scale: &'a str,
    pub dest_dir: Option<&'a str>,
    pub manifest: bool,
    pub prefix: Option<&'a str>,
}

impl Default for ExportOptions<'_> {
    fn default() -> Self {
        Self {
            format: "png",
            scale: "2",
            dest_dir: None,
            manifest: false,
            prefix: None,
        }
    }
}

/// Render and download images for the given node ids.
pub async fn export_images(
    token: &str,
    file_key: &str,
    node_ids: &[String],
    options: &ExportOptions<'_>,
) -> Result<Vec<(String, String)>> {
    let dir = safe_export_dir(options.dest_dir)?;
    let format = normalize_format(options.format);
    let ids_joined = node_ids.join(",");
    let result = files::get_images(
        token,
        file_key,
        &ids_joined,
        None,
        Some(options.scale),
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
    let ext = format_extension(&format);
    let filename_prefix = match options.prefix {
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
                "scale": options.scale,
                "source_url": url,
            }));
            rows.push((node_id.clone(), written));
        }
    }

    if options.manifest {
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
    let dir = safe_export_dir(dest_dir)?;
    let result = files::get_image_fills(token, file_key).await?;
    let images = result
        .get("meta")
        .and_then(|m| m.get("images"))
        .or_else(|| result.get("images"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));

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
