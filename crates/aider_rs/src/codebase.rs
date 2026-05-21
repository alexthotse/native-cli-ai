//! Codebase module - Codebase analysis and parsing

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodebaseError {
    #[error("Directory not found: {0}")]
    DirectoryNotFound(String),
    #[error("Failed to read file: {0}")]
    ReadError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Represents a code file in the codebase
#[derive(Debug, Clone)]
pub struct CodeFile {
    pub path: PathBuf,
    pub content: String,
    pub language: String,
    pub size_bytes: usize,
}

/// Codebase representation
pub struct Codebase {
    root_path: PathBuf,
    files: HashMap<PathBuf, CodeFile>,
}

impl Codebase {
    /// Load a codebase from a directory
    pub fn load(root_path: &Path) -> Result<Self, CodebaseError> {
        if !root_path.exists() || !root_path.is_dir() {
            return Err(CodebaseError::DirectoryNotFound(
                root_path.display().to_string(),
            ));
        }

        let mut files = HashMap::new();
        
        // Walk directory and collect files
        for entry in walkdir::WalkDir::new(root_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| !is_ignored(e.path()))
        {
            let path = entry.path().to_path_buf();
            
            if let Ok(content) = std::fs::read_to_string(&path) {
                let language = detect_language(&path);
                let file = CodeFile {
                    path: path.clone(),
                    content: content.clone(),
                    language,
                    size_bytes: content.len(),
                };
                files.insert(path, file);
            }
        }

        Ok(Self {
            root_path: root_path.to_path_buf(),
            files,
        })
    }

    /// Get a file by path
    pub fn get_file(&self, path: &Path) -> Option<&CodeFile> {
        self.files.get(path)
    }

    /// Search for files containing a pattern
    pub fn search(&self, pattern: &str) -> Vec<&CodeFile> {
        self.files
            .values()
            .filter(|f| f.content.contains(pattern))
            .collect()
    }

    /// Get all files of a specific language
    pub fn files_by_language(&self, language: &str) -> Vec<&CodeFile> {
        self.files
            .values()
            .filter(|f| f.language == language)
            .collect()
    }

    /// Get total number of files
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Get all file paths
    pub fn list_files(&self) -> Vec<&Path> {
        self.files.keys().map(|p| p.as_path()).collect()
    }
}

/// Check if a file should be ignored
fn is_ignored(path: &Path) -> bool {
    let ignored_dirs = [".git", "target", "node_modules", "__pycache__", ".venv"];
    
    path.components()
        .any(|c| ignored_dirs.iter().any(|d| c.as_os_str() == *d))
    || path.extension().map_or(false, |ext| {
        matches!(ext.to_str(), Some("pyc" | "so" | "dll" | "exe"))
    })
}

/// Detect programming language from file extension
fn detect_language(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust".to_string(),
        Some("py") => "python".to_string(),
        Some("js") => "javascript".to_string(),
        Some("ts") => "typescript".to_string(),
        Some("tsx") => "typescript".to_string(),
        Some("jsx") => "javascript".to_string(),
        Some("go") => "go".to_string(),
        Some("rb") => "ruby".to_string(),
        Some("java") => "java".to_string(),
        Some("c") => "c".to_string(),
        Some("cpp") => "cpp".to_string(),
        Some("h") => "c".to_string(),
        Some("hpp") => "cpp".to_string(),
        Some("toml") => "toml".to_string(),
        Some("json") => "json".to_string(),
        Some("yaml") | Some("yml") => "yaml".to_string(),
        Some("md") => "markdown".to_string(),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_codebase_load() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let codebase = Codebase::load(temp_dir.path()).unwrap();
        assert_eq!(codebase.file_count(), 1);
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(detect_language(Path::new("test.rs")), "rust");
        assert_eq!(detect_language(Path::new("test.py")), "python");
        assert_eq!(detect_language(Path::new("test.js")), "javascript");
    }
}