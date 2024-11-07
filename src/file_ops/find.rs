use eyre::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use regex::Regex;
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use walkdir::{DirEntry, WalkDir};

use super::{safe_file_name, safe_parent_dir};
use crate::args::FindMode;
use crossterm::{
    queue,
    style::{self, Attribute, Stylize},
};

#[derive(Debug)]
pub struct SearchResult {
    path: PathBuf,
    line_number: Option<usize>,
    matched_content: Option<String>,
}

pub fn process_find(
    directory: impl AsRef<Path>,
    mode: &FindMode,
    pattern: Option<&str>,
    output: Option<impl AsRef<Path>>,
) -> Result<()> {
    let directory = directory.as_ref();
    println!("Поиск в директории: {}", directory.display());
    println!("Режим поиска: {}", mode);
    if let Some(pattern) = pattern {
        println!("Паттерн: {}", pattern);
    }

    let mp = MultiProgress::new();
    let search_pb = mp.add(create_progress_bar());

    let regex = pattern.and_then(|p| match mode {
        FindMode::Regexp => Some(Regex::new(p).expect("Некорректное регулярное выражение")),
        _ => None,
    });
    let regex = Arc::new(regex);

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

            process_entry(&entry, mode, pattern, &regex)
        })
        .collect();

    search_pb.finish_with_message(format!(
        "Проверено файлов: {}, из них подходящих: {}",
        search_pb.position(),
        results.len()
    ));

    Ok(output_results(&results, output)?)
}

fn create_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

fn process_entry(
    entry: &DirEntry,
    mode: &FindMode,
    pattern: Option<&str>,
    regex: &Arc<Option<Regex>>,
) -> Option<SearchResult> {
    match mode {
        FindMode::FileName => process_filename(entry, pattern?),
        FindMode::Content => process_content(entry, pattern?),
        FindMode::Regexp => process_regex(entry, regex.as_ref().as_ref()?),
        FindMode::Gavriluk => process_gavriluk(entry),
    }
}

fn process_filename(entry: &DirEntry, pattern: &str) -> Option<SearchResult> {
    entry
        .file_name()
        .to_string_lossy()
        .contains(pattern)
        .then(|| SearchResult {
            path: entry.path().to_owned(),
            line_number: None,
            matched_content: None,
        })
}

fn process_content(entry: &DirEntry, pattern: &str) -> Option<SearchResult> {
    if !entry.file_type().is_file() {
        return None;
    }
    let file = File::open(entry.path()).ok()?;
    let reader = BufReader::new(file);
    for (line_num, line) in reader.lines().enumerate() {
        if let Ok(content) = line {
            if content.contains(pattern) {
                return Some(SearchResult {
                    path: entry.path().to_owned(),
                    line_number: Some(line_num + 1),
                    matched_content: Some(content),
                });
            }
        }
    }
    None
}

fn process_regex(entry: &DirEntry, regex: &Regex) -> Option<SearchResult> {
    if !entry.file_type().is_file() {
        return None;
    }
    let file = File::open(entry.path()).ok()?;
    let reader = BufReader::new(file);
    for (line_num, line) in reader.lines().enumerate() {
        if let Ok(content) = line {
            if regex.is_match(&content) {
                return Some(SearchResult {
                    path: entry.path().to_owned(),
                    line_number: Some(line_num + 1),
                    matched_content: Some(content),
                });
            }
        }
    }
    None
}

fn process_gavriluk(entry: &DirEntry) -> Option<SearchResult> {
    let parent = safe_parent_dir(entry.path())?;
    let parent = parent.file_name()?;
    (safe_file_name(entry.path())?
        .match_indices(&*parent.to_string_lossy())
        .count()
        > 1)
    .then(|| SearchResult {
        path: entry.path().to_owned(),
        line_number: None,
        matched_content: None,
    })
}

fn output_results(results: &[SearchResult], output: Option<impl AsRef<Path>>) -> io::Result<()> {
    if let Some(filename) = output {
        let mut file = File::create(filename.as_ref())?;
        write_results(&mut file, results)?;
        println!("Результат записан в файл: {}", filename.as_ref().display());
    } else {
        write_results(&mut io::stdout(), results)?;
    }
    Ok(())
}

fn write_results(writer: &mut impl Write, results: &[SearchResult]) -> io::Result<()> {
    for result in results {
        match (result.line_number, &result.matched_content) {
            (Some(linenum), Some(content)) => {
                // OSC 8 последовательность для создания гиперссылки
                write!(
                    writer,
                    "Файл: \x1b]8;;file://{}\x1b\\\x1b[4:3m{}\x1b[4:0m\x1b]8;;\x1b\\",
                    result.path.to_string_lossy(),
                    result.path.display()
                )?;
                writeln!(writer)?;
                writeln!(writer, "Строка {}: {}", linenum, content)?;
                writeln!(writer, "---")?;
            }
            (_, _) => {
                writeln!(
                    writer,
                    "\x1b]8;;file://{}\x1b\\\x1b[4:3m{}\x1b[4:0m\x1b]8;;\x1b\\",
                    result.path.to_string_lossy(),
                    result.path.display()
                )?;
            }
        }
    }
    Ok(())
}
