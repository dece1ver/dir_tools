use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use regex::Regex;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::time::Duration;
use walkdir::{DirEntry, WalkDir};

use crate::args::FindMode;

#[derive(Debug)]
pub struct SearchResult {
    path: String,
    line_number: Option<usize>,
    matched_content: Option<String>,
}

pub fn process_find(
    directory: &str,
    target: &str,
    mode: &FindMode,
    output: &str,
) -> io::Result<()> {
    let mp = MultiProgress::new();
    let search_pb = mp.add(ProgressBar::new_spinner());
    search_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    search_pb.enable_steady_tick(Duration::from_millis(100));

    let regex = if let FindMode::Regexp = mode {
        Some(Regex::new(target).expect("Invalid regex pattern"))
    } else {
        None
    };

    let results: Vec<SearchResult> = WalkDir::new(directory)
        .into_iter()
        .filter_map(Result::ok)
        .par_bridge()
        .filter_map(|entry| {
            search_pb.set_message(format!(
                "Проверено: {} | Проверка: {}",
                search_pb.position(),
                entry.file_name().to_string_lossy()
            ));
            search_pb.inc(1);

            match mode {
                FindMode::FileName => {
                    if entry.file_name().to_string_lossy().contains(target) {
                        Some(SearchResult {
                            path: entry.path().to_string_lossy().to_string(),
                            line_number: None,
                            matched_content: None,
                        })
                    } else {
                        None
                    }
                }
                FindMode::Content => search_file_content(&entry, target),
                FindMode::Regexp => {
                    if let Some(ref re) = regex {
                        search_file_regex(&entry, re)
                    } else {
                        None
                    }
                }
                FindMode::Gavriluk => {
                    let parent = entry.path().parent().unwrap().file_name()?;
                    if entry
                        .file_name()
                        .to_string_lossy()
                        .match_indices(&*parent.to_string_lossy())
                        .count()
                        > 1
                    {
                        Some(SearchResult {
                            path: entry.path().to_string_lossy().into_owned(),
                            line_number: None,
                            matched_content: None,
                        })
                    } else {
                        None
                    }
                }
            }
        })
        .collect();

    search_pb.finish_with_message(format!(
        "Проверено файлов: {}, из них подходящих: {}",
        search_pb.position(),
        results.len()
    ));

    output_results(&results, output)?;

    Ok(())
}

fn search_file_content(entry: &DirEntry, target: &str) -> Option<SearchResult> {
    if !entry.file_type().is_file() {
        return None;
    }

    if let Ok(file) = File::open(entry.path()) {
        let reader = BufReader::new(file);
        for (line_num, line) in reader.lines().enumerate() {
            if let Ok(content) = line {
                if content.contains(target) {
                    return Some(SearchResult {
                        path: entry.path().to_string_lossy().to_string(),
                        line_number: Some(line_num + 1),
                        matched_content: Some(content),
                    });
                }
            }
        }
    }
    None
}

fn search_file_regex(entry: &DirEntry, regex: &Regex) -> Option<SearchResult> {
    if !entry.file_type().is_file() {
        return None;
    }

    if let Ok(file) = File::open(entry.path()) {
        let reader = BufReader::new(file);
        for (line_num, line) in reader.lines().enumerate() {
            if let Ok(content) = line {
                if regex.is_match(&content) {
                    return Some(SearchResult {
                        path: entry.path().to_string_lossy().to_string(),
                        line_number: Some(line_num + 1),
                        matched_content: Some(content),
                    });
                }
            }
        }
    }
    None
}

fn output_results(results: &[SearchResult], output: &str) -> io::Result<()> {
    if output.is_empty() {
        for result in results {
            match (result.line_number, &result.matched_content) {
                (Some(line_num), Some(content)) => {
                    println!("Файл: {}", result.path);
                    println!("Строка {}: {}", line_num, content);
                    println!("---");
                }
                (_, _) => println!("{}", result.path),
            }
        }
    } else {
        let mut file = File::create(output)?;
        for result in results {
            match (result.line_number, &result.matched_content) {
                (Some(line_num), Some(content)) => {
                    writeln!(file, "Файл: {}", result.path)?;
                    writeln!(file, "Строка {}: {}", line_num, content)?;
                    writeln!(file, "---")?;
                }
                (_, _) => writeln!(file, "{}", result.path)?,
            }
        }
        println!("Результат записан в файл: {}", Path::new(output).display());
    }
    Ok(())
}
