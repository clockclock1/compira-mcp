use std::path::{Path, PathBuf};

/// Download a remote file into the library directory.
/// Returns the relative path (posix-style) written under `root`.
pub async fn download_url(url: &str, root: &Path) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("CompiraMCP/0.1")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?;

    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("download failed {}: {}", resp.status(), url);
    }

    let bytes = resp.bytes().await?;
    let rel = relative_path_from_url(url);
    let dest = root.join(&rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&dest, &bytes)?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

pub async fn download_urls(urls: &[String], root: &Path) -> anyhow::Result<Vec<String>> {
    std::fs::create_dir_all(root)?;
    let mut written = Vec::new();
    for url in urls {
        match download_url(url, root).await {
            Ok(rel) => written.push(rel),
            Err(e) => tracing::warn!("skip url {url}: {e}"),
        }
    }
    if written.is_empty() {
        anyhow::bail!("no files downloaded successfully");
    }
    Ok(written)
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
    let lower = url.to_lowercase();
    let exts = [".vue", ".uvue", ".tsx", ".jsx", ".ts", ".js", ".md"];
    exts.iter().any(|e| lower.split('?').next().unwrap_or("").ends_with(e))
        || lower.contains("raw.githubusercontent.com")
        || lower.contains("jsdelivr.net")
        || lower.contains("unpkg.com")
}
