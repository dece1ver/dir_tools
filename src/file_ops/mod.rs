use walkdir::{DirEntry, WalkDir};

use expose::process_expose;
use flatten::process_flatten;

use crate::args::Operation;

pub mod expose;
pub mod flatten;

pub fn process_directory(operation: &Operation) {
    match operation {
        Operation::Expose { directory, force } => {
            process_expose(directory, *force);
        }
        Operation::Flatten { directory, output, move_files } => {
            process_flatten(directory, output, *move_files);
        }
    }
}

pub fn files_from(directory: &str) -> Vec<DirEntry> {
    WalkDir::new(directory)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| entry.file_type().is_file())
        .collect()
}