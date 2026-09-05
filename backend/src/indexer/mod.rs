use std::path::{Path, PathBuf};

use crate::ai::{self, apply_enrichment};
use crate::config::Config;
use crate::db::{Database, LlmSettings};
use crate::fetcher;
use crate::git;
use crate::parser;

pub async fn sync_library(
    db: &Database,
    task_id: &str,
    library_id: &str,
    repo_url: &str,
    branch: &str,
    local_path: &Path,
    library_name: &str,
) -> anyhow::Result<usize> {
    db.update_library_status(library_id, "syncing", None)?;
    db.update_sync_task(task_id, 5, "Cloning/pulling repository...", None)?;

    git::clone_or_pull(repo_url, branch, local_path)?;
    db.update_sync_task(task_id, 30, "Repository synced, parsing components...", None)?;

    db.clear_library_components(library_id)?;
    let parsed = parser::scan_and_parse_directory(library_id, local_path)?;
    store_parsed(db, task_id, library_id, library_name, parsed, None).await
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

    let mut parsed = Vec::new();
    for rel in relative_paths {
        match parser::parse_component_file(library_id, local_path, rel) {
            Ok(item) => parsed.push(item),
            Err(e) => {
                tracing::warn!("skip {rel}: {e}");
                let _ = db.log("warn", &format!("Skip {rel}: {e}"), Some(library_id));
            }
        }
    }

    if parsed.is_empty() {
        anyhow::bail!("no valid component files to ingest");
    }

    let llm = if use_ai || auto_name {
        config.and_then(|c| db.get_llm_settings(c).ok())
    } else {
        None
    };

    // Auto-name: prefer AI; fallback to component / file names
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

    store_parsed(
        db,
        task_id,
        library_id,
        &display_name,
        parsed,
        enrich_llm.as_ref(),
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

/// Pure AI fetch: natural-language prompt → LLM plans URLs → download → parse (+ AI enrich).
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
    let llm = db.get_llm_settings(config)?;
    if !llm.enabled() {
        anyhow::bail!("AI fetch requires LLM API key (configure in admin settings)");
    }
    let prompt = prompt.trim();
    if prompt.is_empty() {
        anyhow::bail!("prompt is required for AI fetch");
    }

    db.update_library_status(library_id, "syncing", None)?;
    db.update_sync_task(task_id, 5, "AI planning component files...", None)?;

    let plan = ai::plan_fetch_urls(&llm, prompt).await?;
    if let Some(note) = &plan.note {
        let _ = db.log("info", &format!("AI fetch plan: {note}"), Some(library_id));
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

    db.update_sync_task(
        task_id,
        20,
        &format!("AI selected {} file(s), downloading...", plan.urls.len()),
        None,
    )?;
    std::fs::create_dir_all(local_path)?;
    let written = fetcher::download_urls(&plan.urls, local_path).await?;

    db.update_sync_task(task_id, 40, "Download complete, AI parsing...", None)?;
    // auto_name already applied from plan; don't rename again during ingest
    ingest_files(
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
    .await
}

async fn store_parsed(
    db: &Database,
    task_id: &str,
    library_id: &str,
    library_name: &str,
    parsed: Vec<(
        crate::db::Component,
        String,
        String,
        Vec<(String, String)>,
    )>,
    llm: Option<&LlmSettings>,
) -> anyhow::Result<usize> {
    let total = parsed.len();
    db.update_sync_task(
        task_id,
        50,
        &format!("Found {total} components, indexing..."),
        None,
    )?;

    for (i, (mut component, source, mut docs, mut examples)) in parsed.into_iter().enumerate() {
        if let Ok(Some(existing_id)) =
            db.get_component_id_by_path(library_id, &component.file_path)
        {
            component.id = existing_id;
        }

        if let Some(settings) = llm {
            if settings.enabled() {
                db.update_sync_task(
                    task_id,
                    50 + ((i * 40) / total.max(1)) as i32,
                    &format!("AI enriching {}...", component.name),
                    None,
                )?;
                match ai::enrich_component(settings, &component, &source).await {
                    Ok(enrichment) => {
                        apply_enrichment(&mut component, &mut docs, &mut examples, enrichment);
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

        db.upsert_component(&component, &source)?;
        if !docs.is_empty() {
            db.set_component_docs(&component.id, &docs)?;
        }
        if !examples.is_empty() {
            db.set_component_examples(&component.id, &examples)?;
        }
        db.index_component(&component, library_name, &source)?;

        if total > 0 {
            let progress = 50 + ((i + 1) * 45 / total) as i32;
            db.update_sync_task(
                task_id,
                progress,
                &format!("Indexed {}/{} components", i + 1, total),
                None,
            )?;
        }
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

fn sanitize_relative_path(relative: &str) -> anyhow::Result<String> {
    let cleaned = relative.replace('\\', "/");
    if cleaned.contains("..") || cleaned.starts_with('/') {
        anyhow::bail!("invalid path: {relative}");
    }
    Ok(cleaned)
}
