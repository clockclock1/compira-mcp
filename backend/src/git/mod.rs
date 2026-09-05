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
