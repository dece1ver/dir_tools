use eyre::{Result, eyre};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use indicatif::MultiProgress;
use rayon::prelude::*;

use crate::TICK_DURATION;

use super::transfer::{NAME_LOCK_STRIPES, name_stripe, replace_newest_transfer, transfer};
use super::{CustomStyle, count_walk, create_progressbar};

pub fn process_flatten(
    directory: impl AsRef<Path>,
    output: Option<&PathBuf>,
    move_files: bool,
    replace_newest: bool,
) -> Result<()> {
    let directory = directory.as_ref();

    let output_dir = match output {
        Some(path) => path.clone(),
        None => {
            let mut output_path = std::env::current_exe()?;
            output_path.pop();
            output_path.push("flattened_files");
            output_path
        }
    };

    fs::create_dir_all(&output_dir)?;

    let mp = MultiProgress::new();
    let count_pb = mp.add(create_progressbar(
        "{spinner:.green} [{elapsed_precise}] {msg}",
        CustomStyle::Spinner,
        0,
    ));
    count_pb.enable_steady_tick(TICK_DURATION);
    count_pb.set_message("Подсчет файлов...");

    let files = count_walk(directory, &count_pb, |e| e.file_type().is_file())?;
    let errors = Arc::new(Mutex::new(Vec::new()));
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

    let pb = mp.add(create_progressbar(
        "[{elapsed_precise}] {bar:40.green} {pos}/{len} ({eta})",
        CustomStyle::Bar,
        files.len(),
    ));

    let existing_files = Arc::new(Mutex::new(HashSet::with_capacity(files.len())));
    let skipped_files = Arc::new(Mutex::new(Vec::new()));
    let output_dir = Arc::new(output_dir);
    let name_locks: Vec<Mutex<()>> = (0..NAME_LOCK_STRIPES).map(|_| Mutex::new(())).collect();

    let chunk_size = (files.len() / rayon::current_num_threads()).max(1);

    files.par_chunks(chunk_size).for_each(|chunk| {
        let mut local_existing = HashSet::new();

        for entry in chunk {
            let file_name = entry.file_name().to_string_lossy().to_string();
            current_file_pb.set_message(format!("Обработка: {}", file_name));

            let _guard = name_locks[name_stripe(&file_name)]
                .lock()
                .unwrap_or_else(PoisonError::into_inner);

            let result = if replace_newest {
                let dest_path = PathBuf::from(&*output_dir).join(&file_name);
                replace_newest_transfer(entry.path(), &dest_path, move_files)
            } else {
                let mut dest_path = PathBuf::from(&*output_dir).join(&file_name);
                let mut counter = 1;

                while local_existing.contains(&dest_path.to_string_lossy().to_string())
                    || existing_files
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .contains(&dest_path.to_string_lossy().to_string())
                {
                    let new_name = if let Some(ext) = dest_path.extension() {
                        format!(
                            "{} ({}).{}",
                            dest_path
                                .file_stem()
                                .unwrap_or_else(|| dest_path.as_os_str())
                                .to_string_lossy(),
                            counter,
                            ext.to_string_lossy()
                        )
                    } else {
                        format!("{} ({})", file_name, counter)
                    };
                    dest_path = PathBuf::from(&*output_dir).join(new_name);
                    counter += 1;
                }

                transfer(entry.path(), &dest_path, move_files).map(|_| Some(dest_path))
            };

            match result {
                Ok(Some(dest_path)) => {
                    local_existing.insert(dest_path.to_string_lossy().to_string());
                }
                Ok(None) => {
                    skipped_files
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(entry.path().display().to_string());
                }
                Err(e) => {
                    let mut errors = errors.lock().unwrap_or_else(PoisonError::into_inner);
                    errors.push(format!(
                        "Ошибка {} файла {}: {}",
                        if move_files {
                            "перемещения"
                        } else {
                            "копирования"
                        },
                        entry.path().display(),
                        e
                    ));
                }
            }

            pb.inc(1);
        }

        let mut global_existing = existing_files
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        global_existing.extend(local_existing);
    });

    current_file_pb.finish_and_clear();
    pb.finish_with_message("Обработка файлов завершена");

    let skipped = skipped_files.lock().unwrap_or_else(PoisonError::into_inner);
    if existing_files
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_empty()
        && skipped.is_empty()
    {
        eprintln!("Ничего не сделано.");
    } else {
        println!("Все файлы уплощены в {}", output_dir.display());
        if replace_newest && !skipped.is_empty() {
            for file in skipped.iter() {
                eprintln!("Пропущен более старый файл: {file}");
            }
        }
    }
    let errors = errors.lock().unwrap_or_else(PoisonError::into_inner);
    if !errors.is_empty() {
        for err in errors.iter() {
            eprintln!("{err}")
        }
    }

    Ok(())
}
