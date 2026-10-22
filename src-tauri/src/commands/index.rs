//! High-performance in-memory text indexing for code search
//!
//! This module provides:
//! - Inverted index (word -> locations)
//! - Trigram indexing for fuzzy/substring search
//! - Multi-threaded indexing with Rayon

use dashmap::DashMap;
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::State;

const DEFAULT_MAX_FILE_SIZE: u64 = 1024 * 1024; // 1MB
#[allow(dead_code)]
const BINARY_CHECK_BYTES: usize = 8192; // 8KB

/// Options for building the index
#[derive(Debug, Clone, Deserialize)]
pub struct IndexOptions {
    pub file_extensions: Vec<String>,
    pub max_file_size: Option<u64>,
    pub exclude_dirs: Option<Vec<String>>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            file_extensions: vec![],
            max_file_size: Some(DEFAULT_MAX_FILE_SIZE),
            exclude_dirs: Some(vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "build".to_string(),
                "__pycache__".to_string(),
                ".next".to_string(),
            ]),
        }
    }
}

/// Statistics about the index
#[derive(Debug, Clone, Serialize)]
pub struct IndexStats {
    pub total_files: usize,
    pub total_words: usize,
    pub memory_bytes: usize,
    pub root_path: String,
}

/// Location of a word occurrence in a file
#[derive(Debug, Clone)]
struct WordLocation {
    file_id: u32,
}

/// Information stored about each indexed file
#[derive(Debug, Clone)]
struct FileInfo {
    words: HashSet<String>, // Words present in this file (for removal)
}

/// Inverted index for fast text search
pub struct InvertedIndex {
    /// word -> list of `file_id`s it occurs in
    word_index: DashMap<String, Vec<WordLocation>>,
    /// trigram -> list of words containing it (for fuzzy search)
    trigram_index: Option<DashMap<String, Vec<String>>>,
    /// `file_id` -> file path mapping
    files: DashMap<u32, FileInfo>,
    /// path -> `file_id` reverse mapping
    path_to_id: DashMap<String, u32>,
    /// Next available file ID
    next_file_id: AtomicU32,
    /// Root path of the indexed directory
    root_path: std::sync::RwLock<String>,
    /// Word tokenizer regex
    word_regex: Regex,
    /// Total memory estimate (in bytes)
    memory_estimate: AtomicUsize,
}

impl InvertedIndex {
    fn new(enable_trigram: bool) -> Self {
        Self {
            word_index: DashMap::new(),
            trigram_index: if enable_trigram {
                Some(DashMap::new())
            } else {
                None
            },
            files: DashMap::new(),
            path_to_id: DashMap::new(),
            next_file_id: AtomicU32::new(1),
            root_path: std::sync::RwLock::new(String::new()),
            word_regex: Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]{2,}").unwrap(),
            memory_estimate: AtomicUsize::new(0),
        }
    }

    fn set_root(&self, root: String) {
        let mut root_path = self.root_path.write().unwrap();
        *root_path = root;
    }

    fn get_root(&self) -> String {
        self.root_path.read().unwrap().clone()
    }

    /// Get or create file ID for a path
    fn get_or_create_file_id(&self, path: &str) -> u32 {
        *self
            .path_to_id
            .entry(path.to_string())
            .or_insert_with(|| self.next_file_id.fetch_add(1, Ordering::SeqCst))
    }

    /// Add a word to the trigram index
    fn add_trigrams(&self, word: &str) {
        if let Some(ref trigram_index) = self.trigram_index {
            if word.len() >= 3 {
                let word_lower = word.to_lowercase();
                for i in 0..=word_lower.len().saturating_sub(3) {
                    let trigram = &word_lower[i..i + 3];
                    trigram_index
                        .entry(trigram.to_string())
                        .and_modify(|v| {
                            if !v.contains(&word_lower) {
                                v.push(word_lower.clone());
                            }
                        })
                        .or_insert_with(|| vec![word_lower.clone()]);
                }
            }
        }
    }

    /// Index a single file
    fn index_file(&self, file_path: &Path, max_file_size: u64) -> Result<(), String> {
        let path_str = file_path.to_string_lossy().to_string();

        let metadata = fs::metadata(file_path).map_err(|e| e.to_string())?;
        if metadata.len() > max_file_size {
            return Ok(());
        }

        let content = fs::read(file_path).map_err(|e| e.to_string())?;

        if content[..content.len().min(8192)].contains(&0) {
            return Ok(());
        }

        let Ok(text) = String::from_utf8(content) else {
            return Ok(());
        };

        self.remove_file(&path_str);

        let file_id = self.get_or_create_file_id(&path_str);
        let mut file_words: HashSet<String> = HashSet::new();

        // Index each line
        for line in text.lines() {
            for mat in self.word_regex.find_iter(line) {
                let word = mat.as_str();
                let normalized_word = word.to_lowercase();

                self.word_index
                    .entry(normalized_word.clone())
                    .or_default()
                    .push(WordLocation { file_id });

                self.add_trigrams(&normalized_word);
                file_words.insert(normalized_word);

                self.memory_estimate.fetch_add(
                    std::mem::size_of::<WordLocation>() + word.len(),
                    Ordering::Relaxed,
                );
            }
        }

        // Store file info
        self.files.insert(file_id, FileInfo { words: file_words });

        Ok(())
    }

    /// Remove a file from the index
    fn remove_file(&self, path: &str) {
        if let Some((_, file_id)) = self.path_to_id.remove(path) {
            if let Some((_, file_info)) = self.files.remove(&file_id) {
                // Remove all word entries for this file
                for word in file_info.words {
                    if let Some(mut locations) = self.word_index.get_mut(&word) {
                        locations.retain(|loc| loc.file_id != file_id);
                        if locations.is_empty() {
                            drop(locations);
                            self.word_index.remove(&word);
                        }
                    }
                }
            }
        }
    }

    /// Get statistics about the index
    fn stats(&self) -> IndexStats {
        IndexStats {
            total_files: self.files.len(),
            total_words: self.word_index.len(),
            memory_bytes: self.memory_estimate.load(Ordering::Relaxed),
            root_path: self.get_root(),
        }
    }

    /// Clear the entire index
    fn clear(&self) {
        self.word_index.clear();
        if let Some(ref trigram_index) = self.trigram_index {
            trigram_index.clear();
        }
        self.files.clear();
        self.path_to_id.clear();
        self.next_file_id.store(1, Ordering::SeqCst);
        self.memory_estimate.store(0, Ordering::Relaxed);
    }
}

/// Check if a file is binary by looking for null bytes in the first N bytes
#[allow(dead_code)]
fn is_binary_file(path: &Path) -> Result<bool, String> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = [0u8; BINARY_CHECK_BYTES];
    let n = file.read(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf[..n].contains(&0))
}

/// Collect all files to index in a directory
#[allow(clippy::unnecessary_wraps)]
fn collect_files(root: &Path, options: &IndexOptions) -> Result<Vec<std::path::PathBuf>, String> {
    let max_file_size = options.max_file_size.unwrap_or(DEFAULT_MAX_FILE_SIZE);
    let exclude_dirs: HashSet<String> = options
        .exclude_dirs
        .as_ref()
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default();

    let extensions: HashSet<String> = options
        .file_extensions
        .iter()
        .map(|e| e.to_lowercase())
        .collect();

    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(50)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !exclude_dirs.contains(name.as_ref())
        })
    {
        let Ok(entry) = entry else {
            continue;
        };

        if !entry.file_type().is_file() {
            continue;
        }

        // Check file extension
        if !extensions.is_empty() {
            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase);

            if ext.is_none_or(|e| !extensions.contains(&e)) {
                continue;
            }
        }

        // Check file size
        if let Ok(metadata) = entry.metadata() {
            if metadata.len() > max_file_size {
                continue;
            }
        }

        files.push(entry.path().to_path_buf());
    }

    Ok(files)
}

/// Shared index store for Tauri state
pub struct IndexStore {
    index: InvertedIndex,
}

impl IndexStore {
    pub fn new(enable_trigram: bool) -> Self {
        Self {
            index: InvertedIndex::new(enable_trigram),
        }
    }
}

/// Build index for a directory
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn index_build(
    state: State<'_, Arc<IndexStore>>,
    root: String,
    options: Option<IndexOptions>,
) -> Result<IndexStats, String> {
    let options = options.unwrap_or_default();
    let root_path = Path::new(&root);

    if !root_path.exists() {
        return Err(format!("Root path does not exist: {root}"));
    }

    // Clear existing index and set new root
    state.index.clear();
    state.index.set_root(root.clone());

    // Collect files to index
    let files = collect_files(root_path, &options)?;

    // Index files in parallel using Rayon
    let max_file_size = options.max_file_size.unwrap_or(DEFAULT_MAX_FILE_SIZE);

    files.par_iter().for_each(|file_path| {
        let _ = state.index.index_file(file_path, max_file_size);
    });

    Ok(state.index.stats())
}

/// Get index statistics
#[tauri::command]
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
pub fn index_stats(state: State<'_, Arc<IndexStore>>) -> Result<IndexStats, String> {
    Ok(state.index.stats())
}

/// Clear the index
#[tauri::command]
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
pub fn index_clear(state: State<'_, Arc<IndexStore>>) -> Result<(), String> {
    state.index.clear();
    Ok(())
}
