use std::io;
use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

use crate::args::Operation;
use append_folder_name::process_afn;
use expose::process_expose;
use eyre::Result;
use find::process_find;
use flatten::process_flatten;
use rename::process_rename;

pub mod append_folder_name;
pub mod expose;
pub mod find;
pub mod flatten;
pub mod rename;

pub fn process_directory(operation: &Operation) -> Result<()> {
    match operation {
        Operation::Expose { directory, force } => process_expose(directory, *force),
        Operation::Flatten {
            directory,
            output,
            move_files,
        } => process_flatten(directory, output, *move_files),
        Operation::Rename {
            directory,
            target_type,
            find,
            replace,
        } => process_rename(directory, target_type, find, replace),
        Operation::AFN { directory } => process_afn(directory),
        Operation::Find {
            directory,
            mode,
            output,
            pattern,
        } => process_find(directory, mode, pattern.as_deref(), output.as_deref()),
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
