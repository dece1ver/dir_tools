use std::{
    fs,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use super::files_from;

pub fn process_afn(directory: &str) {
    let mp = MultiProgress::new();
    let count_pb = mp.add(ProgressBar::new_spinner());
    count_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Подсчет файлов...")
            .unwrap(),
    );
    count_pb.enable_steady_tick(Duration::from_millis(100));

    let files = files_from(directory);
    let len = files.len() as u64;
    count_pb.finish_with_message(format!("завершен, файлов: {}", files.len()));

    if files.is_empty() {
        eprintln!(
            "Нет доступных файлов для обработки в директории: {}",
            directory
        );
        return;
    }

    let current_file_pb = mp.add(ProgressBar::new(0));
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

    files.par_iter().for_each(|entry| {
        if let Some(parent) = entry.path().parent() {
            let old_name = entry.file_name().to_string_lossy();
            let new_name = format!(
                "{} {}",
                parent.file_name().unwrap().to_string_lossy(),
                entry.file_name().to_string_lossy()
            );
            let dest_path = entry
                .path()
                .to_string_lossy()
                .replace(&old_name.clone().into_owned(), &new_name);
            current_file_pb.set_message(format!("Обработка: {} => {}", &old_name, &new_name));
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
        }
        pb.inc(1);
    });
    pb.finish();
    for err in errors.lock().unwrap().iter() {
        eprint!("{err}");
    }
}
