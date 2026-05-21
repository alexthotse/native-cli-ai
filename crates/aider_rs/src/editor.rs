//! Editor module - Code editing with diff application

use thiserror::Error;
use std::path::Path;

#[derive(Error, Debug)]
pub enum EditorError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Failed to read file: {0}")]
    ReadError(String),
    #[error("Failed to write file: {0}")]
    WriteError(String),
    #[error("Invalid diff format: {0}")]
    InvalidDiff(String),
}

/// Code editor for applying edits to files
pub struct CodeEditor {
    dry_run: bool,
}

impl CodeEditor {
    pub fn new(dry_run: bool) -> Self {
        Self { dry_run }
    }

    /// Read a file's contents
    pub fn read_file(&self, path: &Path) -> Result<String, EditorError> {
        std::fs::read_to_string(path)
            .map_err(|e| EditorError::ReadError(e.to_string()))
    }

    /// Write content to a file
    pub fn write_file(&self, path: &Path, content: &str) -> Result<(), EditorError> {
        if self.dry_run {
            println!("[DRY RUN] Would write to {:?}", path);
            return Ok(());
        }

        std::fs::write(path, content)
            .map_err(|e| EditorError::WriteError(e.to_string()))
    }

    /// Apply a unified diff to a file
    pub fn apply_diff(&self, path: &Path, diff: &str) -> Result<(), EditorError> {
        let original = self.read_file(path)?;
        
        // Parse and apply the diff
        // TODO: Implement proper diff parsing using diffy crate
        let modified = diffy::apply(&original, diff)
            .map_err(|e| EditorError::InvalidDiff(e.to_string()))?;

        self.write_file(path, &modified)
    }

    /// Make an edit to a file (search and replace)
    pub fn make_edit(
        &self,
        path: &Path,
        search: &str,
        replace: &str,
    ) -> Result<(), EditorError> {
        let content = self.read_file(path)?;
        
        if !content.contains(search) {
            return Err(EditorError::InvalidDiff(
                "Search string not found in file".to_string(),
            ));
        }

        let new_content = content.replace(search, replace);
        self.write_file(path, &new_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_code_editor_read_write() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Hello, World!").unwrap();

        let editor = CodeEditor::new(true); // dry run
        
        let content = editor.read_file(temp_file.path()).unwrap();
        assert!(content.contains("Hello, World!"));
    }
}