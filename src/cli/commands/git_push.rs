//! Git push operations for version bumps before bundling.

use crate::error::{BundlerError, Result};
use crate::cli::RuntimeConfig;
use std::path::Path;

/// Push uncommitted version changes to GitHub main branch.
///
/// This function:
/// 1. Opens the git repository at `repo_path`
/// 2. Checks if there are uncommitted changes
/// 3. If yes, commits them with a version bump message
/// 4. Pushes to origin/main
///
/// ## Authentication Requirements
///
/// **SSH (Recommended)**:
/// ```bash
/// eval "$(ssh-agent -s)"
/// ssh-add ~/.ssh/id_rsa
/// ```
///
/// **HTTPS**:
/// ```bash
/// git config --global credential.helper store
/// ```
pub async fn push_version_changes<P: AsRef<Path>>(
    repo_path: P,
    runtime_config: &RuntimeConfig,
) -> Result<()> {
    use kodegen_tools_git::{
        open_repo, is_clean, add, AddOpts, commit, CommitOpts, push, PushOpts,
    };

    let repo_path = repo_path.as_ref();
    
    // Step 1: Open repository
    runtime_config.verbose_println(&format!(
        "   Opening git repository: {}",
        repo_path.display()
    ))?;
    
    let repo = open_repo(repo_path)
        .await
        .map_err(|e| BundlerError::Cli(crate::error::CliError::ExecutionFailed {
            command: "open_repo".to_string(),
            reason: format!("Not a git repository: {}", e),
        }))?
        .map_err(|e| BundlerError::Cli(crate::error::CliError::ExecutionFailed {
            command: "open_repo".to_string(),
            reason: format!("Not a git repository: {}", e),
        }))?;
    
    // Step 2: Check if working directory is clean
    runtime_config.verbose_println("   Checking for uncommitted changes...")?;
    
    let is_clean = is_clean(&repo)
        .await
        .map_err(|e| BundlerError::Cli(crate::error::CliError::ExecutionFailed {
            command: "is_clean".to_string(),
            reason: format!("Failed to check git status: {}", e),
        }))?;
    
    if is_clean {
        runtime_config.verbose_println("   ✓ No uncommitted changes to push")?;
        return Ok(());
    }
    
    // Step 3: Commit changes
    runtime_config.verbose_println("   Committing version bump changes...")?;
    
    // Stage all changes
    add(
        repo.clone(),
        AddOpts {
            paths: vec![std::path::PathBuf::from(".")],
            update_only: false,
            force: false,
        },
    )
    .await
    .map_err(|e| BundlerError::Cli(crate::error::CliError::ExecutionFailed {
        command: "git add".to_string(),
        reason: format!("Failed to stage changes: {}", e),
    }))?;
    
    // Create commit
    commit(
        repo.clone(),
        CommitOpts {
            message: "chore: version bump for release".to_string(),
            amend: false,
            all: false,
            author: None,
            committer: None,
        },
    )
    .await
    .map_err(|e| BundlerError::Cli(crate::error::CliError::ExecutionFailed {
        command: "git commit".to_string(),
        reason: format!("Failed to commit changes: {}", e),
    }))?;
    
    runtime_config.verbose_println("   ✓ Committed version changes")?;
    
    // Step 4: Push to remote
    runtime_config.verbose_println("   Pushing to origin/main...")?;
    
    let result = push(
        &repo,
        PushOpts {
            remote: "origin".to_string(),
            refspecs: Vec::new(),  // Push current branch
            force: false,
            tags: false,
            timeout_secs: Some(300),  // 5 minute timeout
        },
    )
    .await
    .map_err(|e| BundlerError::Cli(crate::error::CliError::ExecutionFailed {
        command: "git push".to_string(),
        reason: format!(
            "Failed to push to origin. Ensure git authentication is configured:\n\
             \n\
             SSH: eval \"$(ssh-agent -s)\" && ssh-add ~/.ssh/id_rsa\n\
             HTTPS: git config --global credential.helper store\n\
             \n\
             Error: {}",
            e
        ),
    }))?;
    
    runtime_config.success_println(&format!(
        "   ✓ Pushed {} commit(s) to origin",
        result.commits_pushed
    ))?;
    
    Ok(())
}
