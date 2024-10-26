use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;

use crate::args::RenameTarget;
use crate::file_ops::files_from;

use super::{dirs_from, entries_from};

pub fn process_rename(directory: &str, target_type: &RenameTarget, find: &str, replace: &str) {
    let mp = MultiProgress::new();

    let objects_title = match target_type {
        RenameTarget::Dirs => "директорий",
        RenameTarget::Files => "файлов",
        RenameTarget::Both => "объектов",
    };

    let count_pb = mp.add(ProgressBar::new_spinner());
    count_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Подсчет {msg}...")
            .unwrap(),
    );
    count_pb.set_message(objects_title);
    count_pb.enable_steady_tick(Duration::from_millis(100));

    let entries = match target_type {
        RenameTarget::Dirs => dirs_from(directory),
        RenameTarget::Files => files_from(directory),
        RenameTarget::Both => entries_from(directory),
    };

    count_pb.finish_and_clear();

    if entries.is_empty() {
        eprintln!(
            "Нет доступных {} для обработки в директории: {}",
            objects_title, directory
        );
        return;
    }

    let len = entries.len() as u64;
    let current_file_pb = mp.add(ProgressBar::new(len));
    current_file_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );

    let pb = mp.add(ProgressBar::new(len));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.green}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=> "),
    );

    let processed = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(Mutex::new(Vec::new()));

    entries
        .par_iter()
        .filter_map(|entry| {
            entry
                .path()
                .file_stem()
                .map(|stem| (entry, stem.to_string_lossy().into_owned()))
        })
        .filter(|(_, name)| name.contains(find))
        .for_each(|(entry, old_name)| {
            let new_name = old_name.replace(find, replace);
            current_file_pb.set_message(format!("Обработка: {} => {}", &old_name, &new_name));

            let dest_path = entry
                .path()
                .with_file_name(&new_name)
                .to_string_lossy()
                .into_owned();

            if let Err(e) = fs::rename(entry.path(), &dest_path) {
                errors.lock().unwrap().push(format!(
                    "Ошибка переименования \"{}{}\": {}",
                    entry.path().display(),
                    if entry.path().is_dir() { "\\" } else { "" },
                    e
                ));
            } else {
                processed.fetch_add(1, Ordering::Relaxed);
            }

            pb.inc(1);
        });

    current_file_pb.finish_and_clear();
    pb.finish_with_message(format!("Обработка {} завершена", objects_title));

    let objects_title = match target_type {
        RenameTarget::Dirs => "Директорий",
        RenameTarget::Files => "Файлов",
        RenameTarget::Both => "Объектов",
    };

    println!(
        "{} переименовано: {}",
        objects_title,
        processed.load(Ordering::Relaxed)
    );

    let errors = errors.lock().unwrap();
    for error in errors.iter() {
        eprintln!("{}", error);
    }
}
