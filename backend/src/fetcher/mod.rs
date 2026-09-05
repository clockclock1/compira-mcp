use std::path::{Path, PathBuf};
use std::time::Duration;

/// Download a remote file into the library directory.
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
    if status.as_u16() == 404 {
        for alt in alternate_github_ref_urls(&url) {
            tracing::info!("404 on {url}, retrying {alt}");
            match Box::pin(download_url_once(&alt, root)).await {
                Ok(rel) => return Ok(rel),
                Err(e) => tracing::warn!("retry failed {alt}: {e}"),
            }
        }
    }
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

async fn download_url_once(url: &str, root: &Path) -> anyhow::Result<String> {
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

pub async fn download_urls(urls: &[String], root: &Path) -> anyhow::Result<Vec<String>> {
    std::fs::create_dir_all(root)?;
    let mut written = Vec::new();
    let mut errors = Vec::new();
    for url in urls {
        match download_url(url, root).await {
            Ok(rel) => {
                tracing::info!("downloaded {url} -> {rel}");
                written.push(rel);
            }
            Err(e) => {
                tracing::warn!("skip url {url}: {e}");
                errors.push(format!("{url}: {e}"));
            }
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

/// Convert github.com/.../blob/... to raw.githubusercontent.com/...
pub fn normalize_component_url(url: &str) -> String {
    let url = url.trim();
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        // owner/repo/blob/ref/path
        let parts: Vec<&str> = rest.splitn(5, '/').collect();
        if parts.len() == 5 && parts[2] == "blob" {
            return format!(
                "https://raw.githubusercontent.com/{}/{}/{}/{}",
                parts[0], parts[1], parts[3], parts[4]
            );
        }
    }
    url.to_string()
}

/// If a raw.githubusercontent.com URL 404s on a common wrong default branch, try alternates.
fn alternate_github_ref_urls(url: &str) -> Vec<String> {
    const MARKER: &str = "raw.githubusercontent.com/";
    let Some(idx) = url.find(MARKER) else {
        return Vec::new();
    };
    let rest = &url[idx + MARKER.len()..];
    let mut parts = rest.splitn(4, '/');
    let (Some(owner), Some(repo), Some(git_ref), Some(path)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Vec::new();
    };

    let alts: &[&str] = match git_ref {
        "main" => &["dev", "master", "next"],
        "master" => &["main", "dev"],
        "dev" => &["main", "master"],
        _ => return Vec::new(),
    };
    alts.iter()
        .map(|r| format!("https://raw.githubusercontent.com/{owner}/{repo}/{r}/{path}"))
        .collect()
}

fn relative_path_from_url(url: &str) -> PathBuf {
    // Prefer last path segments that look like a component path
    let without_query = url.split('?').next().unwrap_or(url);
    let path = without_query
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    // raw.githubusercontent.com/{owner}/{repo}/{ref}/path...
    if let Some(rest) = path.strip_prefix("raw.githubusercontent.com/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 4 {
            return PathBuf::from(parts[3..].join("/"));
        }
    }

    // cdn.jsdelivr.net/gh/owner/repo@version/path
    if let Some(rest) = path.strip_prefix("cdn.jsdelivr.net/gh/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 {
            // parts[0]=owner, parts[1]=repo@version, rest=path
            return PathBuf::from(parts[2..].join("/"));
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
    let exts = [".vue", ".uvue", ".tsx", ".jsx", ".ts", ".js", ".md", ".css", ".scss"];
    let has_ext = exts.iter().any(|e| path.ends_with(e));
    let host_ok = lower.contains("raw.githubusercontent.com")
        || lower.contains("jsdelivr.net")
        || lower.contains("unpkg.com")
        || lower.contains("github.com/");
    has_ext && host_ok
}
