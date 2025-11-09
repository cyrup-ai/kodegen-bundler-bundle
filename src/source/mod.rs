//! Source repository resolution

use std::path::PathBuf;
use crate::error::Result;

pub enum RepositorySource {
    Local(PathBuf),
    GitHub { org: String, repo: String },
    GitHubUrl(String),
}

impl RepositorySource {
    pub fn parse(source: &str) -> Result<Self> {
        // GitHub org/repo: contains '/', no '://', not path-like
        if source.contains('/') && !source.contains("://") 
            && !source.starts_with('.') && !source.starts_with('/') {
            let parts: Vec<&str> = source.split('/').collect();
            if parts.len() == 2 {
                return Ok(Self::GitHub {
                    org: parts[0].to_string(),
                    repo: parts[1].to_string(),
                });
            }
        }
        
        // GitHub URL
        if source.starts_with("http://") || source.starts_with("https://") {
            return Ok(Self::GitHubUrl(source.to_string()));
        }
        
        // Local path
        Ok(Self::Local(PathBuf::from(source)))
    }
    
    pub async fn resolve(&self) -> Result<PathBuf> {
        match self {
            Self::Local(path) => {
                // Local path: read Cargo.toml to get repository URL, then clone from GitHub
                if !path.exists() {
                    return Err(crate::error::BundlerError::Cli(
                        crate::error::CliError::InvalidArguments {
                            reason: format!("Path does not exist: {}", path.display()),
                        }
                    ));
                }
                
                let cargo_toml_path = path.join("Cargo.toml");
                if !cargo_toml_path.exists() {
                    return Err(crate::error::BundlerError::Cli(
                        crate::error::CliError::InvalidArguments {
                            reason: format!("Cargo.toml not found at {}", cargo_toml_path.display()),
                        }
                    ));
                }
                
                // Read repository URL from Cargo.toml
                let manifest = crate::metadata::load_manifest(&cargo_toml_path)?;
                let repo_url = manifest.metadata.repository.ok_or_else(|| {
                    crate::error::BundlerError::Cli(crate::error::CliError::InvalidArguments {
                        reason: format!(
                            "No repository field in Cargo.toml at {}. \
                             Bundler requires a GitHub repository URL to clone from.",
                            cargo_toml_path.display()
                        ),
                    })
                })?;
                
                // Clone from GitHub to tmp
                clone_repo(&repo_url).await
            }
            Self::GitHub { org, repo } => {
                let url = format!("https://github.com/{}/{}.git", org, repo);
                clone_repo(&url).await
            }
            Self::GitHubUrl(url) => clone_repo(url).await,
        }
    }
}

async fn clone_repo(url: &str) -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir()
        .join(format!("kodegen-bundle-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir).await?;
    
    let temp_dir_str = temp_dir.to_str().ok_or_else(|| {
        crate::error::BundlerError::Cli(crate::error::CliError::InvalidArguments {
            reason: format!("Temp directory path contains invalid UTF-8: {}", temp_dir.display()),
        })
    })?;
    
    let output = tokio::process::Command::new("git")
        .args(["clone", "--depth=1", url, temp_dir_str])
        .output()
        .await?;
    
    if !output.status.success() {
        return Err(crate::error::BundlerError::Cli(
            crate::error::CliError::ExecutionFailed {
                command: "git clone".to_string(),
                reason: String::from_utf8_lossy(&output.stderr).to_string(),
            }
        ));
    }
    
    Ok(temp_dir)
}
