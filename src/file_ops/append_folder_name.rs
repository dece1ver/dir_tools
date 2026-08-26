use eyre::{Result, eyre};
use indicatif::MultiProgress;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::TICK_DURATION;

use super::{CustomStyle, create_progressbar, files_from};

pub fn process_afn(directory: impl AsRef<Path>, delimiter: &str) -> Result<()> {
    let directory = directory.as_ref();
    let mp = MultiProgress::new();
    let count_pb = mp.add(create_progressbar(
        "{spinner:.green} [{elapsed_precise}] {msg}",
        CustomStyle::Spinner,
        0,
    ));
    count_pb.enable_steady_tick(TICK_DURATION);
    count_pb.set_message("Подсчет файлов...");
    let files = files_from(directory)?;
    let len = files.len() as u64;
    count_pb.finish_and_clear();

    if files.is_empty() {
        return Err(eyre!(
            "Нет доступных файлов для обработки в директории: {}",
            directory.display()
        ));
    }

    let current_file_pb = mp.add(create_progressbar(
        "{spinner:.green} {msg}",
        CustomStyle::Spinner,
        0,
    ));
    current_file_pb.set_length(0);
    let pb = mp.add(create_progressbar(
        "[{elapsed_precise}] {bar:40.green} {pos}/{len} ({eta})",
        CustomStyle::Bar,
        len,
    ));
    let processed = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(Mutex::new(Vec::new()));

    files.par_iter().for_each(|entry| {
        if let Some(parent) = entry.path().parent()
            && let Some(parent_name) = parent.file_name()
        {
            let old_name = entry.file_name().to_string_lossy();
            let new_name = format!(
                "{}{}{}",
                parent_name.to_string_lossy(),
                delimiter,
                entry.file_name().to_string_lossy()
            );
            let dest_path = entry
                .path()
                .to_string_lossy()
                .replace(&old_name.clone().into_owned(), &new_name);
            current_file_pb.set_message(format!("Обработка: {old_name} => {new_name}"));
            if let Err(e) = fs::rename(entry.path(), &dest_path) {
                errors
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(format!(
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
    for err in errors.lock().unwrap_or_else(PoisonError::into_inner).iter() {
        eprint!("{err}");
    }
    Ok(())
}
