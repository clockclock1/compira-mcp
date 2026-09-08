use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

/// Download a remote file into the library directory (single URL, no mirror rewriting).
/// Returns the relative path (posix-style) written under `root`.
pub async fn download_url(url: &str, root: &Path) -> anyhow::Result<String> {
    let url = normalize_component_url(url);
    if !is_allowed_component_url(&url) {
        anyhow::bail!("url not allowed (need raw .vue/.ts/.js etc.): {url}");
    }

    let client = reqwest::Client::builder()
        .user_agent("CompiraMCP/0.1 (+https://github.com/clockclock1/compira-mcp)")
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(Duration::from_secs(60))
        .build()?;

    let resp = client
        .get(&url)
        .header(reqwest::header::ACCEPT, "*/*")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("request error for {url}: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(120).collect();
        anyhow::bail!("download failed {status} {url}: {snippet}");
    }
    let bytes = resp.bytes().await?;
    if bytes.is_empty() {
        anyhow::bail!("empty body: {url}");
    }
    write_download(root, &url, &bytes)
}

fn write_download(root: &Path, url: &str, bytes: &[u8]) -> anyhow::Result<String> {
    let rel = relative_path_from_url(url);
    let dest = root.join(&rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&dest, bytes)?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

pub async fn download_urls(
    urls: &[String],
    root: &Path,
    concurrency: usize,
) -> anyhow::Result<Vec<String>> {
    std::fs::create_dir_all(root)?;
    let concurrency = concurrency.clamp(1, 64);
    let sem = Arc::new(Semaphore::new(concurrency));
    let root = root.to_path_buf();
    let mut joins = Vec::with_capacity(urls.len());

    for url in urls {
        let url = url.clone();
        let root = root.clone();
        let sem = sem.clone();
        joins.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok()?;
            match download_url(&url, &root).await {
                Ok(rel) => {
                    tracing::info!("downloaded {url} -> {rel}");
                    Some(Ok(rel))
                }
                Err(e) => {
                    tracing::warn!("skip url {url}: {e}");
                    Some(Err(format!("{url}: {e}")))
                }
            }
        }));
    }

    let mut written = Vec::new();
    let mut errors = Vec::new();
    for join in joins {
        match join.await {
            Ok(Some(Ok(rel))) => written.push(rel),
            Ok(Some(Err(e))) => errors.push(e),
            Ok(None) => errors.push("download slot closed".into()),
            Err(e) => errors.push(format!("download task join: {e}")),
        }
    }

    if written.is_empty() {
        let detail = if errors.is_empty() {
            "no urls provided".into()
        } else {
            errors.join(" | ")
        };
        anyhow::bail!("no files downloaded successfully ({detail})");
    }
    Ok(written)
}

/// Convert github / gitlab / gitee blob pages to raw file URLs (same file, not a mirror).
pub fn normalize_component_url(url: &str) -> String {
    let url = url.trim();

    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        let parts: Vec<&str> = rest.splitn(5, '/').collect();
        if parts.len() == 5 && parts[2] == "blob" {
            return format!(
                "https://raw.githubusercontent.com/{}/{}/{}/{}",
                parts[0], parts[1], parts[3], parts[4]
            );
        }
    }

    if let Some(rest) = url
        .strip_prefix("https://gitlab.com/")
        .or_else(|| url.strip_prefix("http://gitlab.com/"))
    {
        if let Some((owner_repo, after)) = rest.split_once("/-/blob/") {
            if let Some((git_ref, path)) = after.split_once('/') {
                return format!("https://gitlab.com/{owner_repo}/-/raw/{git_ref}/{path}");
            }
        }
    }

    if let Some(rest) = url
        .strip_prefix("https://gitee.com/")
        .or_else(|| url.strip_prefix("http://gitee.com/"))
    {
        let parts: Vec<&str> = rest.splitn(5, '/').collect();
        if parts.len() == 5 && parts[2] == "blob" {
            return format!(
                "https://gitee.com/{}/{}/raw/{}/{}",
                parts[0], parts[1], parts[3], parts[4]
            );
        }
    }

    url.to_string()
}

fn relative_path_from_url(url: &str) -> PathBuf {
    let without_query = url.split('?').next().unwrap_or(url);
    let path = without_query
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    if let Some(rest) = path.strip_prefix("raw.githubusercontent.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 4 {
            return PathBuf::from(parts[3..].join("/"));
        }
    }

    for prefix in ["cdn.jsdelivr.net/gh/", "fastly.jsdelivr.net/gh/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() >= 3 {
                return PathBuf::from(parts[2..].join("/"));
            }
        }
    }

    if let Some(idx) = path.find("/-/raw/") {
        let after = &path[idx + "/-/raw/".len()..];
        if let Some((_, file_path)) = after.split_once('/') {
            return PathBuf::from(file_path);
        }
    }

    if let Some(rest) = path.strip_prefix("gitee.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 5 && parts[2] == "raw" {
            return PathBuf::from(parts[4..].join("/"));
        }
    }

    let name = Path::new(without_query)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("component.vue");
    PathBuf::from("fetched").join(name)
}

pub fn is_allowed_component_url(url: &str) -> bool {
    let lower = normalize_component_url(url).to_lowercase();
    let path = lower.split('?').next().unwrap_or("");
    let exts = [
        ".vue", ".uvue", ".tsx", ".jsx", ".ts", ".js", ".svelte", ".astro", ".dart", ".wxml",
        ".html", ".md", ".css", ".scss", ".wxss", ".json",
    ];
    let has_ext = exts.iter().any(|e| path.ends_with(e));
    let host_ok = lower.contains("raw.githubusercontent.com")
        || lower.contains("jsdelivr.net")
        || lower.contains("unpkg.com")
        || lower.contains("github.com/")
        || lower.contains("gitlab.com/")
        || lower.contains("gitee.com/");
    has_ext && host_ok
}
