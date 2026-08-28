use std::env;
use std::io::{self, stderr};
use std::path::{Path, PathBuf};

use crossterm::ansi_support::supports_ansi;
use crossterm::tty::IsTty;
use indicatif::{ProgressBar, ProgressStyle};
use lock::process_lock;
use tree::process_tree;
use walkdir::{DirEntry, WalkDir};

use crate::args::Operation;
use crate::{ANSI_PROGRESS_CHARS, PROGRESS_CHARS, TICK_CHARS, TICK_DURATION};
use append_folder_name::process_afn;
use expose::process_expose;
use eyre::{Context, Result};
use find::process_find;
use flatten::process_flatten;
use merge::process_merge;
use rename::process_rename;

pub mod append_folder_name;
pub mod expose;
pub mod find;
pub mod flatten;
pub mod lock;
pub mod merge;
pub mod rename;
pub mod transfer;
pub mod tree;

// Функция для преобразования относительных путей в абсолютные
fn resolve_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let current_dir = env::current_dir().wrap_err("Не удалось получить текущую директорию")?;
        Ok(current_dir.join(path))
    }
}

pub fn process_directory(operation: &Operation) -> Result<()> {
    match operation {
        Operation::Expose { directory, force } => {
            let abs_dir = resolve_path(directory)?;
            process_expose(&abs_dir, *force)
        }
        Operation::Flatten {
            directory,
            output,
            move_files,
            replace_newest,
        } => {
            let abs_dir = resolve_path(directory)?;
            let abs_output = output
                .as_ref()
                .map(|o| resolve_path(Path::new(o)).unwrap_or_else(|_| PathBuf::from(o)));
            process_flatten(&abs_dir, abs_output.as_ref(), *move_files, *replace_newest)
        }
        Operation::Rename {
            directory,
            target_type,
            find,
            replace,
        } => {
            let abs_dir = resolve_path(directory)?;
            process_rename(&abs_dir, target_type, find, replace)
        }
        Operation::AddParentDir {
            directory,
            delimiter,
        } => {
            let abs_dir = resolve_path(directory)?;
            process_afn(&abs_dir, delimiter)
        }
        Operation::Find {
            directory,
            mode,
            pattern,
            output,
        } => {
            let abs_dir = resolve_path(directory)?;
            let abs_output = if let Some(out) = output {
                Some(resolve_path(out)?)
            } else {
                None
            };
            process_find(&abs_dir, mode, pattern.as_deref(), abs_output.as_deref())
        }
        Operation::Lock { path, timer, mode } => {
            let abs_path = resolve_path(path)?;
            process_lock(&abs_path, timer, mode)
        }
        Operation::Merge {
            directory,
            output,
            from,
            move_files,
        } => {
            let abs_dir = resolve_path(directory)?;
            let abs_output = output
                .as_ref()
                .map(|o| resolve_path(Path::new(o)).unwrap_or_else(|_| PathBuf::from(o)));
            process_merge(&abs_dir, abs_output.as_ref(), from.as_deref(), *move_files)
        }
        Operation::Tree {
            directory,
            show_content,
            max_depth,
            full_content,
            show_hidden,
        } => {
            let abs_dir = resolve_path(directory)?;
            process_tree(
                &abs_dir,
                *show_content,
                *max_depth,
                *full_content,
                *show_hidden,
            )
        }
    }
}

pub fn _ensure_directory(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = path.as_ref();
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(path.to_owned())
}

pub fn safe_parent_dir(path: impl AsRef<Path>) -> Option<PathBuf> {
    path.as_ref().parent().map(PathBuf::from)
}

pub fn safe_file_name(path: impl AsRef<Path>) -> Option<String> {
    path.as_ref()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

pub fn count_walk(
    directory: &Path,
    pb: &ProgressBar,
    predicate: impl Fn(&DirEntry) -> bool,
) -> io::Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    for entry in WalkDir::new(directory)
        .into_iter()
        .filter_map(Result::ok)
    {
        if predicate(&entry) {
            pb.inc(1);
            let rel = entry
                .path()
                .strip_prefix(directory)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            pb.set_message(format_live_count(&rel, pb.position()));
            entries.push(entry);
        }
    }
    Ok(entries)
}

fn format_live_count(path: &str, count: u64) -> String {
    let width = terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80);
    let msg_width = width.saturating_sub(22);
    let count_str = count.to_string();
    let count_w = unicode_width::UnicodeWidthStr::width(count_str.as_str());
    let path_max = msg_width.saturating_sub(count_w + 1);
    let truncated = truncate_left(path, path_max);
    let used = unicode_width::UnicodeWidthStr::width(truncated.as_str());
    let count_pad = msg_width.saturating_sub(used);
    format!("{truncated}{count_str:>count_pad$}")
}

fn truncate_left(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    if max <= 3 {
        return "...".to_string();
    }
    let cut = s.len() - (max - 3);
    let cut = s
        .char_indices()
        .map(|(i, _)| i)
        .find(|&i| i >= cut)
        .unwrap_or(s.len());
    format!("...{}", &s[cut..])
}

trait IntoProgressLen {
    fn into_progress_len(self) -> u64;
}

impl IntoProgressLen for u64 {
    fn into_progress_len(self) -> u64 {
        self
    }
}

impl IntoProgressLen for i32 {
    fn into_progress_len(self) -> u64 {
        if self >= 0 { self as u64 } else { 0u64 }
    }
}

impl IntoProgressLen for usize {
    fn into_progress_len(self) -> u64 {
        self as u64
    }
}

fn create_progressbar<I>(template: &str, style: CustomStyle, len: I) -> ProgressBar
where
    I: IntoProgressLen,
{
    let pb = ProgressBar::new(len.into_progress_len());

    #[cfg(windows)]
    let is_modern_windows_terminal =
        std::env::var("WT_SESSION").is_ok() || std::env::var("TERM_PROGRAM").is_ok();

    #[cfg(not(windows))]
    let is_modern_windows_terminal = true;

    if stderr().is_tty() && supports_ansi() && is_modern_windows_terminal {
        match style {
            CustomStyle::Spinner => {
                let style = ProgressStyle::default_spinner()
                    .template(template)
                    .unwrap_or_else(|_| ProgressStyle::default_spinner());
                pb.set_style(style);
            }
            CustomStyle::Bar => {
                let style = ProgressStyle::default_bar()
                    .template(template)
                    .unwrap_or_else(|_| ProgressStyle::default_bar());
                pb.set_style(style.progress_chars(ANSI_PROGRESS_CHARS));
            }
        }
    } else {
        match style {
            CustomStyle::Spinner => {
                let style = ProgressStyle::default_spinner()
                    .template(template)
                    .unwrap_or_else(|_| ProgressStyle::default_spinner());
                pb.set_style(style.tick_chars(TICK_CHARS).progress_chars(PROGRESS_CHARS));
            }
            CustomStyle::Bar => {
                let style = ProgressStyle::default_bar()
                    .template(template)
                    .unwrap_or_else(|_| ProgressStyle::default_bar());
                pb.set_style(style.tick_chars(TICK_CHARS).progress_chars(PROGRESS_CHARS));
            }
        }
    }

    pb.enable_steady_tick(TICK_DURATION);
    pb
}

enum CustomStyle {
    Spinner,
    Bar,
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn live_count_fits_within_terminal_and_right_aligns_count() {
        let path = "0312211958/9/GILZA-AR112-01-002M8L.1UST.PBN";
        let count = 37507_u64;

        let rendered = format_live_count(path, count);

        let total_width = UnicodeWidthStr::width(rendered.as_str());
        assert!(total_width <= 80, "вывод шире терминала: {total_width}");

        assert!(
            rendered.trim_end().ends_with("37507"),
            "счётчик должен быть в хвосте: {rendered}"
        );
    }

    #[test]
    fn live_count_truncates_long_cyrillic_path() {
        let path = "очень/длинный/путь/на/кириллице/с/файлом.txt";
        let count = 123_u64;

        let rendered = format_live_count(path, count);

        let total_width = UnicodeWidthStr::width(rendered.as_str());
        assert!(total_width <= 80, "вывод шире терминала: {total_width}");
        assert!(rendered.starts_with("..."), "путь должен быть обрезан слева");
    }
}
