//! Git integration module - Git repository operations

use thiserror::Error;
use std::path::{Path, PathBuf};

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Repository not found: {0}")]
    RepoNotFound(String),
    #[error("Git operation failed: {0}")]
    OperationFailed(String),
    #[error("No changes to commit")]
    NoChanges,
}

/// Git repository wrapper
pub struct GitRepo {
    path: PathBuf,
}

impl GitRepo {
    /// Open a git repository at the given path
    pub fn open(path: &Path) -> Result<Self, GitError> {
        // TODO: Use git2 crate for full git integration
        if !path.exists() {
            return Err(GitError::RepoNotFound(path.display().to_string()));
        }

        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Get the current branch name
    pub fn current_branch(&self) -> Result<String, GitError> {
        // Simplified implementation - would use git2 in production
        std::process::Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .output()
            .map_err(|e| GitError::OperationFailed(e.to_string()))
            .and_then(|output| {
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
                } else {
                    Err(GitError::OperationFailed(
                        String::from_utf8_lossy(&output.stderr).to_string(),
                    ))
                }
            })
    }

    /// Check if there are uncommitted changes
    pub fn has_changes(&self) -> Result<bool, GitError> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .arg("status")
            .arg("--porcelain")
            .output()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
    }

    /// Add files to staging
    pub fn add(&self, files: &[&str]) -> Result<(), GitError> {
        let mut cmd = std::process::Command::new("git");
        cmd.arg("-C").arg(&self.path).arg("add");

        for file in files {
            cmd.arg(file);
        }

        cmd.output()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        Ok(())
    }

    /// Commit changes with a message
    pub fn commit(&self, message: &str) -> Result<(), GitError> {
        if !self.has_changes()? {
            return Err(GitError::NoChanges);
        }

        std::process::Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .arg("commit")
            .arg("-m")
            .arg(message)
            .output()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        Ok(())
    }

    /// Create a diff of changes
    pub fn diff(&self, staged: bool) -> Result<String, GitError> {
        let mut cmd = std::process::Command::new("git");
        cmd.arg("-C").arg(&self.path).arg("diff");

        if staged {
            cmd.arg("--cached");
        }

        let output = cmd
            .output()
            .map_err(|e| GitError::OperationFailed(e.to_string()))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_git_repo_open() {
        let temp_dir = TempDir::new().unwrap();
        
        // Initialize a git repo
        std::process::Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        let repo = GitRepo::open(temp_dir.path()).unwrap();
        assert_eq!(repo.path, temp_dir.path());
    }
}