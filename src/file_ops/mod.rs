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
use rename::process_rename;

pub mod append_folder_name;
pub mod expose;
pub mod find;
pub mod flatten;
pub mod lock;
pub mod rename;
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
        } => {
            let abs_dir = resolve_path(directory)?;
            let abs_output = output
                .as_ref()
                .map(|o| resolve_path(Path::new(o)).unwrap_or_else(|_| PathBuf::from(o)));
            process_flatten(&abs_dir, abs_output.as_ref(), *move_files)
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

pub fn files_from(directory: impl AsRef<Path>) -> io::Result<Vec<DirEntry>> {
    collect_entries(directory, |entry| entry.file_type().is_file())
}

pub fn dirs_from(directory: impl AsRef<Path>) -> io::Result<Vec<DirEntry>> {
    collect_entries(directory, |entry| entry.file_type().is_dir())
}

pub fn entries_from(directory: impl AsRef<Path>) -> io::Result<Vec<DirEntry>> {
    collect_entries(directory, |_| true)
}

fn collect_entries(
    directory: impl AsRef<Path>,
    predicate: impl Fn(&DirEntry) -> bool,
) -> io::Result<Vec<DirEntry>> {
    WalkDir::new(directory.as_ref())
        .into_iter()
        .filter_map(|e| match e {
            Ok(entry) if predicate(&entry) => Some(Ok(entry)),
            Ok(_) => None,
            Err(e) => Some(Err(io::Error::new(io::ErrorKind::Other, e))),
        })
        .collect()
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
        if self >= 0 {
            self as u64
        } else {
            0u64
        }
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
                pb.set_style(ProgressStyle::default_spinner().template(template).unwrap());
            }
            CustomStyle::Bar => {
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template(template)
                        .unwrap()
                        .progress_chars(ANSI_PROGRESS_CHARS),
                );
            }
        }
    } else {
        match style {
            CustomStyle::Spinner => {
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template(template)
                        .unwrap()
                        .tick_chars(TICK_CHARS)
                        .progress_chars(PROGRESS_CHARS),
                );
            }
            CustomStyle::Bar => {
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template(template)
                        .unwrap()
                        .tick_chars(TICK_CHARS)
                        .progress_chars(PROGRESS_CHARS),
                );
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
