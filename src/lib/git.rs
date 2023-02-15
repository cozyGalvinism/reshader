use std::path::Path;

use crate::prelude::{ReShaderError, ReShaderResult};

pub fn pull(repository_path: &Path, branch: Option<&str>) -> ReShaderResult<()> {
    let name = repository_path.file_name().unwrap().to_str().unwrap();
    if !repository_path.exists() {
        return Err(ReShaderError::RepositoryNotFound(name.to_string()));
    }
    let repo = git2::Repository::open(repository_path)?;
    let mut remote = repo.find_remote("origin")?;
    let mut fetch_options = git2::FetchOptions::new();

    let refspec = if let Some(branch) = branch {
        branch.to_string()
    } else {
        let head = repo.head()?;
        let branch = head.shorthand().unwrap();
        branch.to_string()
    };

    remote.fetch(&[&refspec], Some(&mut fetch_options), None)?;
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let remote_commit = repo.reference_to_annotated_commit(&fetch_head)?;

    let (analysis, _) = repo.merge_analysis(&[&remote_commit])?;
    if analysis.is_fast_forward() {
        let refname = format!("refs/heads/{}", &refspec);
        match repo.find_reference(&refname) {
            Ok(mut r) => {
                let name = match r.name() {
                    Some(s) => s.to_string(),
                    None => String::from_utf8_lossy(r.name_bytes()).to_string(),
                };
                r.set_target(
                    remote_commit.id(),
                    &format!("ff: {} -> {}", &name, remote_commit.id()),
                )?;
                repo.set_head(&name)?;
                repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
            }
            Err(_) => {
                return Err(ReShaderError::BranchNotFound(refspec, name.to_string()));
            }
        };
    } else if analysis.is_normal() {
        let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
        let local_tree = repo.find_commit(head_commit.id())?.tree()?;
        let remote_tree = repo.find_commit(remote_commit.id())?.tree()?;
        let ancestor = repo
            .find_commit(repo.merge_base(head_commit.id(), remote_commit.id())?)?
            .tree()?;
        let mut index = repo.merge_trees(&ancestor, &local_tree, &remote_tree, None)?;

        if index.has_conflicts() {
            repo.checkout_index(Some(&mut index), None)?;
            return Err(ReShaderError::MergeConflict(refspec, name.to_string()));
        }
        let result_tree = repo.find_tree(index.write_tree_to(&repo)?)?;
        let msg = format!("merge: {} -> {}", head_commit.id(), remote_commit.id());
        let sig = repo.signature()?;
        let local_commit = repo.find_commit(head_commit.id())?;
        let remote_commit = repo.find_commit(remote_commit.id())?;

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &msg,
            &result_tree,
            &[&local_commit, &remote_commit],
        )?;
        repo.checkout_head(None)?;
    }

    Ok(())
}
