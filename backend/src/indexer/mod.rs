use std::path::{Path, PathBuf};

use crate::ai::{self, apply_enrichment};
use crate::config::Config;
use crate::db::{Database, LlmSettings, SyncSettings};
use crate::fetcher;
use crate::git;
use crate::parser::{self, ParsedBundle};

fn resolve_sync(db: &Database, config: &Config) -> SyncSettings {
    db.get_sync_settings(config).unwrap_or(SyncSettings {
        max_jobs: config.max_jobs,
        parse_concurrency: config.parse_concurrency,
        ingest_batch_size: config.ingest_batch_size,
        download_concurrency: config.download_concurrency,
        fetch_max_attempts: config.fetch_max_attempts,
    })
}

pub async fn sync_library(
    db: &Database,
    config: &Config,
    task_id: &str,
    library_id: &str,
    repo_url: &str,
    branch: &str,
    local_path: &Path,
    library_name: &str,
) -> anyhow::Result<usize> {
    db.update_library_status(library_id, "syncing", None)?;
    db.update_sync_task(task_id, 5, "Cloning/pulling repository...", None)?;

    let repo_url = repo_url.to_string();
    let branch = branch.to_string();
    let local_path_buf = local_path.to_path_buf();
    tokio::task::spawn_blocking(move || git::clone_or_pull(&repo_url, &branch, &local_path_buf))
        .await??;

    db.update_sync_task(task_id, 25, "Repository synced, scanning files...", None)?;
    db.clear_library_components(library_id)?;

    ingest_directory_chunked(
        db,
        config,
        task_id,
        library_id,
        local_path,
        library_name,
        true,
        None,
    )
    .await
}

/// Sync Git remote, or re-index local upload/fetch directories so users can refresh anytime.
pub async fn sync_or_reindex(
    db: &Database,
    config: &Config,
    task_id: &str,
    library_id: &str,
    source_type: &str,
    repo_url: &str,
    branch: &str,
    local_path: &Path,
    library_name: &str,
) -> anyhow::Result<usize> {
    if source_type == "git" || source_type.is_empty() {
        return sync_library(
            db,
            config,
            task_id,
            library_id,
            repo_url,
            branch,
            local_path,
            library_name,
        )
        .await;
    }

    // upload / fetch: re-scan local tree
    if !local_path.exists() {
        anyhow::bail!("本地目录不存在，请先上传或 AI 拉取组件");
    }

    db.update_library_status(library_id, "syncing", None)?;
    db.update_sync_task(
        task_id,
        10,
        "Re-indexing local component files...",
        None,
    )?;
    db.clear_library_components(library_id)?;

    // upload may include .ts/.js; fetch/git-style trees prefer UI extensions
    let bulk = source_type != "upload";
    ingest_directory_chunked(
        db,
        config,
        task_id,
        library_id,
        local_path,
        library_name,
        bulk,
        None,
    )
    .await
}

/// Incrementally ingest specific relative files (upload / fetch). Optionally AI-enrich.
pub async fn ingest_files(
    db: &Database,
    config: Option<&Config>,
    task_id: &str,
    library_id: &str,
    local_path: &Path,
    library_name: &str,
    relative_paths: &[String],
    use_ai: bool,
    auto_name: bool,
) -> anyhow::Result<usize> {
    db.update_library_status(library_id, "syncing", None)?;
    db.update_sync_task(
        task_id,
        10,
        &format!("Parsing {} uploaded/fetched file(s)...", relative_paths.len()),
        None,
    )?;

    let library_id_owned = library_id.to_string();
    let root = local_path.to_path_buf();
    let paths = relative_paths.to_vec();
    let parse_conc = config
        .map(|c| resolve_sync(db, c).parse_concurrency)
        .unwrap_or(0);
    let parsed = tokio::task::spawn_blocking(move || {
        parser::parse_paths_parallel(&library_id_owned, &root, &paths, parse_conc)
    })
    .await?;

    if parsed.is_empty() {
        anyhow::bail!("no valid component files to ingest");
    }

    let llm = if use_ai || auto_name {
        config.and_then(|c| db.get_llm_settings(c).ok())
    } else {
        None
    };

    if auto_name {
        let names: Vec<String> = parsed.iter().map(|(c, _, _, _)| c.name.clone()).collect();
        let files: Vec<String> = relative_paths.to_vec();
        let new_name = if let Some(settings) = llm.as_ref().filter(|s| s.enabled()) {
            db.update_sync_task(task_id, 15, "AI naming library...", None)?;
            match ai::suggest_library_name(settings, &names, &files).await {
                Ok(n) => n,
                Err(e) => {
                    tracing::warn!("AI naming failed: {e}");
                    fallback_library_name(&names, &files)
                }
            }
        } else {
            fallback_library_name(&names, &files)
        };
        let _ = db.update_library_name(library_id, &new_name);
        let _ = db.log(
            "info",
            &format!("Library auto-named: {new_name}"),
            Some(library_id),
        );
    }

    let enrich_llm = if use_ai { llm } else { None };
    let display_name = if auto_name {
        db.get_library(library_id)?
            .map(|l| l.name)
            .unwrap_or_else(|| library_name.to_string())
    } else {
        library_name.to_string()
    };

    let batch_size = config
        .map(|c| resolve_sync(db, c).ingest_batch_size)
        .unwrap_or(250);
    store_parsed_batched(
        db,
        task_id,
        library_id,
        &display_name,
        parsed,
        enrich_llm.as_ref(),
        batch_size,
    )
    .await
}

fn fallback_library_name(names: &[String], files: &[String]) -> String {
    if let Some(n) = names.first() {
        if names.len() == 1 {
            return n.clone();
        }
        return format!("{} 等 {} 个组件", n, names.len());
    }
    files
        .first()
        .and_then(|f| Path::new(f).file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("上传组件")
        .to_string()
}

/// Pure AI fetch: natural-language / repo URL → download or clone → parse (+ optional AI enrich).
pub async fn fetch_and_ingest(
    db: &Database,
    config: &Config,
    task_id: &str,
    library_id: &str,
    local_path: &Path,
    library_name: &str,
    prompt: &str,
    auto_name: bool,
) -> anyhow::Result<usize> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        anyhow::bail!("prompt is required for AI fetch");
    }

    db.update_library_status(library_id, "syncing", None)?;
    db.update_sync_task(task_id, 5, "Analyzing request...", None)?;
    let sync = resolve_sync(db, config);
    let max_attempts = sync.fetch_max_attempts.clamp(1, 12);

    let mut failure_ctx: Option<String> = None;

    // Direct whole-repo from URL in prompt (no LLM required). On failure, fall through to AI replan if LLM is on.
    if let Some((repo_url, branch, whole)) = ai::detect_repo_intent(prompt) {
        if whole {
            let llm = db.get_llm_settings(config).ok();
            match ingest_whole_repo(
                db,
                config,
                task_id,
                library_id,
                local_path,
                library_name,
                &repo_url,
                &branch,
                auto_name,
                llm.as_ref(),
            )
            .await
            {
                Ok(n) => return Ok(n),
                Err(e) => {
                    let msg = e.to_string();
                    if llm.as_ref().is_some_and(|s| s.enabled()) {
                        tracing::warn!("whole-repo clone failed, will ask AI for other links: {msg}");
                        failure_ctx = Some(format!(
                            "克隆仓库失败 ({repo_url}@{branch}): {msg}\n请改为 mode=urls，给出其他可直连的组件文件地址（不要重复失败仓库的同一路径）。"
                        ));
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    let llm = db.get_llm_settings(config)?;
    if !llm.enabled() {
        anyhow::bail!("AI fetch requires LLM API key (configure in admin settings)");
    }

    let mut last_error = String::new();

    for attempt in 1..=max_attempts {
        db.update_sync_task(
            task_id,
            8 + attempt as i32,
            &format!("AI 查找下载链接（第 {attempt}/{max_attempts} 次）..."),
            None,
        )?;

        let plan = match ai::plan_fetch_urls_with_context(&llm, prompt, failure_ctx.as_deref()).await
        {
            Ok(p) => p,
            Err(e) => {
                last_error = e.to_string();
                failure_ctx = Some(format!("AI planning failed: {last_error}"));
                tracing::warn!("AI plan attempt {attempt}/{max_attempts} failed: {last_error}");
                continue;
            }
        };

        if let Some(note) = &plan.note {
            let _ = db.log(
                "info",
                &format!("AI fetch plan [{attempt}/{max_attempts}]: {note}"),
                Some(library_id),
            );
        }

        let mut resolved_name = library_name.to_string();
        if auto_name {
            if let Some(n) = plan
                .library_name
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                let _ = db.update_library_name(library_id, n);
                resolved_name = n.to_string();
                let _ = db.log("info", &format!("Library auto-named: {n}"), Some(library_id));
            }
        }

        if plan.mode == "repo" || plan.whole_repo {
            let repo_url = plan
                .repo_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let Some(repo_url) = repo_url else {
                last_error = "AI requested repo mode but repo_url is empty".into();
                failure_ctx = Some(last_error.clone());
                continue;
            };
            let branch = plan
                .branch
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("main");
            match ingest_whole_repo(
                db,
                config,
                task_id,
                library_id,
                local_path,
                &resolved_name,
                repo_url,
                branch,
                false,
                Some(&llm),
            )
            .await
            {
                Ok(n) => return Ok(n),
                Err(e) => {
                    last_error = e.to_string();
                    failure_ctx = Some(format!(
                        "repo clone failed ({repo_url}@{branch}): {last_error}\n请改用 mode=urls，提供其他可直连文件地址，不要重复同一仓库克隆。"
                    ));
                    tracing::warn!(
                        "whole-repo attempt {attempt}/{max_attempts} failed: {last_error}"
                    );
                    continue;
                }
            }
        }

        if plan.urls.is_empty() {
            last_error = "AI returned empty urls".into();
            failure_ctx = Some(last_error.clone());
            continue;
        }

        let _ = db.log(
            "info",
            &format!(
                "AI fetch urls [{attempt}/{max_attempts}] ({}): {}",
                plan.urls.len(),
                plan.urls.join(" , ")
            ),
            Some(library_id),
        );

        db.update_sync_task(
            task_id,
            20,
            &format!(
                "第 {attempt}/{max_attempts} 次：下载 {} 个文件…",
                plan.urls.len()
            ),
            None,
        )?;
        std::fs::create_dir_all(local_path)?;

        match fetcher::download_urls(&plan.urls, local_path, sync.download_concurrency).await {
            Ok(written) => {
                db.update_sync_task(task_id, 40, "Download complete, parsing...", None)?;
                return ingest_files(
                    db,
                    Some(config),
                    task_id,
                    library_id,
                    local_path,
                    &resolved_name,
                    &written,
                    true,
                    false,
                )
                .await;
            }
            Err(e) => {
                last_error = e.to_string();
                let tried = plan.urls.join("\n");
                failure_ctx = Some(format!(
                    "下载失败（每个链接只试一次，不会自动换镜像）:\n{last_error}\n\n已失败的 URL:\n{tried}\n\n请另找其他可用直链（可换站点/路径/包源），禁止重复上述地址。"
                ));
                tracing::warn!("download attempt {attempt}/{max_attempts} failed: {last_error}");
            }
        }
    }

    anyhow::bail!("AI fetch failed after {max_attempts} attempt(s): {last_error}")
}

async fn ingest_whole_repo(
    db: &Database,
    config: &Config,
    task_id: &str,
    library_id: &str,
    local_path: &Path,
    library_name: &str,
    repo_url: &str,
    branch: &str,
    auto_name: bool,
    llm: Option<&LlmSettings>,
) -> anyhow::Result<usize> {
    db.update_sync_task(
        task_id,
        15,
        &format!("Cloning whole repository {repo_url} ({branch})..."),
        None,
    )?;
    let _ = db.log(
        "info",
        &format!("Whole-repo import: {repo_url}@{branch}"),
        Some(library_id),
    );

    let repo_url_owned = repo_url.to_string();
    let branch_owned = branch.to_string();
    let local_path_buf = local_path.to_path_buf();
    let used_branch = tokio::task::spawn_blocking(move || {
        git::clone_or_pull_try_branches(&repo_url_owned, &branch_owned, &local_path_buf)
    })
    .await??;

    {
        let conn_err = (|| -> anyhow::Result<()> {
            if let Some(mut lib) = db.get_library(library_id)? {
                lib.repo_url = repo_url.to_string();
                lib.branch = used_branch.clone();
                db.update_library(&lib)?;
            }
            Ok(())
        })();
        if let Err(e) = conn_err {
            tracing::warn!("failed to update library repo_url: {e}");
        }
    }

    let mut display_name = library_name.to_string();
    if auto_name {
        db.update_sync_task(task_id, 35, "Scanning file list for naming...", None)?;
        let root = local_path.to_path_buf();
        let sample_paths = tokio::task::spawn_blocking(move || {
            let mut paths = parser::collect_component_paths(&root, true);
            paths.truncate(40);
            paths
        })
        .await?;
        let names: Vec<String> = sample_paths
            .iter()
            .filter_map(|p| Path::new(p).file_stem()?.to_str().map(|s| s.to_string()))
            .collect();
        let suggested = if let Some(settings) = llm.filter(|s| s.enabled()) {
            db.update_sync_task(task_id, 40, "AI naming library...", None)?;
            match ai::suggest_library_name(settings, &names, &sample_paths).await {
                Ok(n) => n,
                Err(_) => names
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Git 组件库".into()),
            }
        } else {
            names
                .first()
                .map(|n| format!("{n} 组件库"))
                .unwrap_or_else(|| "Git 组件库".into())
        };
        let _ = db.update_library_name(library_id, &suggested);
        display_name = suggested;
    }

    db.clear_library_components(library_id)?;
    ingest_directory_chunked(
        db,
        config,
        task_id,
        library_id,
        local_path,
        &display_name,
        true,
        None,
    )
    .await
}

/// Stream: collect paths → parse/write in batches (bounded memory).
async fn ingest_directory_chunked(
    db: &Database,
    config: &Config,
    task_id: &str,
    library_id: &str,
    local_path: &Path,
    library_name: &str,
    bulk: bool,
    llm: Option<&LlmSettings>,
) -> anyhow::Result<usize> {
    db.update_sync_task(task_id, 40, "Collecting component file paths...", None)?;
    let root = local_path.to_path_buf();
    let paths = tokio::task::spawn_blocking(move || parser::collect_component_paths(&root, bulk))
        .await?;

    if paths.is_empty() {
        anyhow::bail!(
            "未找到可解析的组件文件（.vue/.uvue/.tsx/.jsx/.svelte/.astro/.dart/.wxml 或 *.component.ts）"
        );
    }

    let total = paths.len();
    let sync = resolve_sync(db, config);
    let batch_size = sync.ingest_batch_size;
    let parse_conc = sync.parse_concurrency;
    db.update_sync_task(
        task_id,
        45,
        &format!(
            "Indexing {total} components (batch {batch_size}, parse threads {})...",
            if parse_conc == 0 {
                "auto".into()
            } else {
                parse_conc.to_string()
            }
        ),
        None,
    )?;

    let mut processed = 0usize;
    for (chunk_idx, chunk) in paths.chunks(batch_size).enumerate() {
        let library_id_owned = library_id.to_string();
        let root = local_path.to_path_buf();
        let chunk_owned = chunk.to_vec();
        let mut parsed = tokio::task::spawn_blocking(move || {
            parser::parse_paths_parallel(&library_id_owned, &root, &chunk_owned, parse_conc)
        })
        .await?;

        if let Some(settings) = llm.filter(|s| s.enabled()) {
            for (component, source, docs, examples) in &mut parsed {
                match ai::enrich_component(settings, component, source).await {
                    Ok(enrichment) => {
                        apply_enrichment(component, docs, examples, enrichment);
                    }
                    Err(e) => {
                        tracing::warn!("AI enrich failed for {}: {e}", component.name);
                    }
                }
            }
        }

        let batch = parsed;
        let n = batch.len();
        db.upsert_components_batch(library_name, &batch)?;
        processed += n;

        let progress = 45 + ((processed * 50) / total.max(1)) as i32;
        // Update progress every batch (not every file) to keep DB responsive
        if chunk_idx % 1 == 0 || processed == total {
            db.update_sync_task(
                task_id,
                progress.min(95),
                &format!("Indexed {processed}/{total} components"),
                None,
            )?;
        }
        // Yield so HTTP/MCP can run between batches
        tokio::task::yield_now().await;
    }

    let count = db.count_library_components(library_id)?;
    db.update_library_status(library_id, "ready", Some(count))?;
    db.update_sync_task(
        task_id,
        100,
        &format!("Ingest completed: {processed} components processed, library has {count}"),
        Some("completed"),
    )?;
    db.log(
        "info",
        &format!("Library '{library_name}' ingested {processed} components (total {count})"),
        Some(library_id),
    )?;

    Ok(processed)
}

async fn store_parsed_batched(
    db: &Database,
    task_id: &str,
    library_id: &str,
    library_name: &str,
    mut parsed: Vec<ParsedBundle>,
    llm: Option<&LlmSettings>,
    batch_size: usize,
) -> anyhow::Result<usize> {
    let total = parsed.len();
    db.update_sync_task(
        task_id,
        50,
        &format!("Found {total} components, indexing..."),
        None,
    )?;

    if let Some(settings) = llm.filter(|s| s.enabled()) {
        for (i, (component, source, docs, examples)) in parsed.iter_mut().enumerate() {
            db.update_sync_task(
                task_id,
                50 + ((i * 30) / total.max(1)) as i32,
                &format!("AI enriching {} ({}/{})...", component.name, i + 1, total),
                None,
            )?;
            match ai::enrich_component(settings, component, source).await {
                Ok(enrichment) => {
                    apply_enrichment(component, docs, examples, enrichment);
                }
                Err(e) => {
                    tracing::warn!("AI enrich failed for {}: {e}", component.name);
                    let _ = db.log(
                        "warn",
                        &format!("AI enrich failed for {}: {e}", component.name),
                        Some(library_id),
                    );
                }
            }
        }
    }

    let mut processed = 0usize;
    for chunk in parsed.chunks(batch_size) {
        db.upsert_components_batch(library_name, chunk)?;
        processed += chunk.len();
        let progress = 80 + ((processed * 15) / total.max(1)) as i32;
        db.update_sync_task(
            task_id,
            progress.min(95),
            &format!("Indexed {processed}/{total} components"),
            None,
        )?;
        tokio::task::yield_now().await;
    }

    let count = db.count_library_components(library_id)?;
    db.update_library_status(library_id, "ready", Some(count))?;
    db.update_sync_task(
        task_id,
        100,
        &format!("Ingest completed: {total} components processed, library has {count}"),
        Some("completed"),
    )?;
    db.log(
        "info",
        &format!("Library '{library_name}' ingested {total} components (total {count})"),
        Some(library_id),
    )?;

    Ok(total)
}

pub fn ensure_local_dir(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}

pub fn write_upload_file(root: &Path, relative: &str, bytes: &[u8]) -> anyhow::Result<PathBuf> {
    let safe = sanitize_relative_path(relative)?;
    let dest = root.join(&safe);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&dest, bytes)?;
    Ok(dest)
}

/// Extract a ZIP into `uploads/<stem>/` and return relative paths of component files.
pub fn extract_zip_upload(root: &Path, archive_name: &str, bytes: &[u8]) -> anyhow::Result<Vec<String>> {
    use std::io::{Cursor, Read};

    let stem = Path::new(archive_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("archive");
    let stem = sanitize_archive_stem(stem);
    let base = format!("uploads/{stem}");

    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| anyhow::anyhow!("invalid zip archive: {e}"))?;

    let mut component_paths = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| anyhow::anyhow!("zip entry {i}: {e}"))?;
        if file.is_dir() {
            continue;
        }
        let Some(enclosed) = file.enclosed_name() else {
            continue;
        };
        let inner = enclosed.to_string_lossy().replace('\\', "/");
        if should_skip_zip_entry(&inner) {
            continue;
        }
        let rel = format!("{base}/{inner}");
        let safe = sanitize_relative_path(&rel)?;
        let dest = root.join(&safe);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        std::fs::write(&dest, &buf)?;

        let ext = Path::new(&inner)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if crate::parser::is_component_extension(ext) {
            component_paths.push(safe);
        }
    }

    if component_paths.is_empty() {
        anyhow::bail!(
            "zip 中未找到可解析的组件文件（.vue/.svelte/.astro/.tsx/.jsx/.dart/.wxml/.ts/.js 等）"
        );
    }
    Ok(component_paths)
}

pub fn is_zip_filename(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn sanitize_archive_stem(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "archive".into()
    } else {
        cleaned
    }
}

fn should_skip_zip_entry(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if lower.contains("node_modules/")
        || lower.contains("/.git/")
        || lower.starts_with(".git/")
        || lower.contains("__macosx/")
        || lower.ends_with(".ds_store")
    {
        return true;
    }
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    const KEEP: &[&str] = &[
        "vue", "uvue", "tsx", "jsx", "ts", "js", "svelte", "astro", "dart", "wxml", "html", "md",
        "css", "scss", "less", "wxss", "wxs", "json",
    ];
    !KEEP.contains(&ext.as_str())
}

fn sanitize_relative_path(relative: &str) -> anyhow::Result<String> {
    let cleaned = relative.replace('\\', "/");
    if cleaned.contains("..") || cleaned.starts_with('/') {
        anyhow::bail!("invalid path: {relative}");
    }
    Ok(cleaned)
}
