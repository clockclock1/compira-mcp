use git2::{FetchOptions, Repository};
use std::path::Path;

pub fn clone_or_pull(repo_url: &str, branch: &str, local_path: &Path) -> anyhow::Result<()> {
    // create_library may mkdir an empty folder — only pull when it's a real git repo
    let is_repo = local_path.join(".git").is_dir() || Repository::open(local_path).is_ok();
    if is_repo {
        pull(local_path, branch)
    } else {
        if local_path.exists() {
            std::fs::remove_dir_all(local_path)?;
        }
        clone(repo_url, branch, local_path)
    }
}

/// Try preferred branch then common defaults (main/master/dev).
pub fn clone_or_pull_try_branches(
    repo_url: &str,
    preferred_branch: &str,
    local_path: &Path,
) -> anyhow::Result<String> {
    let mut tried = Vec::new();
    let mut branches = vec![preferred_branch.to_string()];
    for b in ["main", "master", "dev"] {
        if !branches.iter().any(|x| x == b) {
            branches.push(b.to_string());
        }
    }
    let mut last_err = None;
    for branch in branches {
        tried.push(branch.clone());
        match clone_or_pull(repo_url, &branch, local_path) {
            Ok(()) => return Ok(branch),
            Err(e) => {
                tracing::warn!("clone/pull {repo_url}@{branch} failed: {e}");
                last_err = Some(e);
                if local_path.exists() {
                    let _ = std::fs::remove_dir_all(local_path);
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("clone failed")))
        .map_err(|e| anyhow::anyhow!("clone failed for branches [{}]: {e}", tried.join(",")))
}

fn clone(repo_url: &str, branch: &str, local_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut builder = git2::build::RepoBuilder::new();
    builder.branch(branch);
    builder.clone(repo_url, local_path)?;
    Ok(())
}

fn pull(local_path: &Path, branch: &str) -> anyhow::Result<()> {
    let repo = Repository::open(local_path)?;
    let mut remote = repo.find_remote("origin")?;

    let mut fo = FetchOptions::new();
    fo.remote_callbacks(git2::RemoteCallbacks::new());
    remote.fetch(&[branch], Some(&mut fo), None)?;

    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let annotated = repo.reference_to_annotated_commit(&fetch_head)?;
    let (analysis, _) = repo.merge_analysis(&[&annotated])?;

    if analysis.is_up_to_date() {
        return Ok(());
    }

    if analysis.is_fast_forward() {
        let refname = format!("refs/heads/{branch}");
        let mut reference = repo.find_reference(&refname)?;
        reference.set_target(annotated.id(), "Fast-forward")?;
        repo.set_head(&refname)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
    } else {
        repo.merge(&[&annotated], None, None)?;
        repo.cleanup_state()?;
    }

    Ok(())
}

pub fn remove_repo(local_path: &Path) -> anyhow::Result<()> {
    if local_path.exists() {
        std::fs::remove_dir_all(local_path)?;
    }
    Ok(())
}
