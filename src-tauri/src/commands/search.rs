use serde::{Deserialize, Serialize};
use std::path::Path;

use sidex_workspace::search::{
    search_files as crate_search_files, FileSearchOptions as CrateFileSearchOptions, SearchEngine,
    SearchOptions as CrateSearchOptions, SearchQuery as CrateSearchQuery,
};

#[derive(Debug, Clone, Serialize)]
pub struct FileMatch {
    pub path: String,
    pub name: String,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextMatch {
    pub path: String,
    pub line_number: usize,
    pub line_content: String,
    pub column: usize,
    pub match_length: usize,
}

#[derive(Debug, Deserialize)]
pub struct SearchFileOptions {
    pub max_results: Option<usize>,
    pub include_hidden: Option<bool>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SearchTextOptions {
    pub max_results: Option<usize>,
    pub case_sensitive: Option<bool>,
    pub is_regex: Option<bool>,
    #[allow(dead_code)]
    pub include_hidden: Option<bool>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub max_file_size: Option<u64>,
}

// scmCode: both searches walk the folder on the blocking pool. As plain `fn`s they
// ran on the main thread and froze the window for a large search (docs/audit TR-16).
#[tauri::command]
pub async fn search_files(
    root: String,
    pattern: String,
    options: Option<SearchFileOptions>,
) -> Result<Vec<FileMatch>, String> {
    crate::scmcode_blocking::off_main(move || search_files_blocking(&root, &pattern, options)).await
}

#[allow(clippy::unnecessary_wraps)]
fn search_files_blocking(
    root: &str,
    pattern: &str,
    options: Option<SearchFileOptions>,
) -> Result<Vec<FileMatch>, String> {
    let crate_opts = options.map(|o| CrateFileSearchOptions {
        max_results: o.max_results,
        include_hidden: o.include_hidden,
        include: o.include,
        exclude: o.exclude,
    });

    let matches = crate_search_files(Path::new(root), pattern, crate_opts.as_ref());

    Ok(matches
        .into_iter()
        .map(|m| FileMatch {
            path: m.path,
            name: m.name,
            score: m.score,
        })
        .collect())
}

#[tauri::command]
pub async fn search_text(
    root: String,
    query: String,
    options: Option<SearchTextOptions>,
) -> Result<Vec<TextMatch>, String> {
    crate::scmcode_blocking::off_main(move || search_text_blocking(&root, query, options.as_ref()))
        .await
}

fn search_text_blocking(
    root: &str,
    query: String,
    options: Option<&SearchTextOptions>,
) -> Result<Vec<TextMatch>, String> {
    let opts_ref = options;
    let query_len = query.len();

    let crate_query = CrateSearchQuery {
        pattern: query,
        is_regex: opts_ref.and_then(|o| o.is_regex).unwrap_or(false),
        case_sensitive: opts_ref.and_then(|o| o.case_sensitive).unwrap_or(false),
        whole_word: false,
        max_results: opts_ref.and_then(|o| o.max_results),
    };

    let crate_opts = CrateSearchOptions {
        include_patterns: opts_ref.and_then(|o| o.include.clone()).unwrap_or_default(),
        exclude_patterns: opts_ref.and_then(|o| o.exclude.clone()).unwrap_or_default(),
        max_results: opts_ref.and_then(|o| o.max_results),
        max_file_size: opts_ref.and_then(|o| o.max_file_size),
        context_lines: None,
    };

    let results = SearchEngine::search_with_options(Path::new(root), &crate_query, &crate_opts)
        .map_err(|e| e.to_string())?;

    Ok(results
        .into_iter()
        .map(|r| TextMatch {
            path: r.path.to_string_lossy().into_owned(),
            line_number: r.line_number,
            line_content: r.line_text,
            column: r.match_start,
            match_length: r.match_end.saturating_sub(r.match_start).max(query_len),
        })
        .collect())
}
