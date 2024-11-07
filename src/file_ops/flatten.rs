use eyre::{eyre, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;

use crate::file_ops::files_from;

pub fn process_flatten(
    directory: impl AsRef<Path>,
    output: &Option<String>,
    move_files: bool,
) -> Result<()> {
    let directory = directory.as_ref();

    let output_dir = match output {
        Some(path) => path,
        None => {
            let mut output_path = std::env::current_exe()?;
            output_path.pop();
            output_path.push("flattened_files");
            &output_path.to_str().unwrap().to_string()
        }
    };

    fs::create_dir_all(output_dir)?;

    let mp = MultiProgress::new();
    let count_pb = mp.add(ProgressBar::new_spinner());
    count_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Подсчет файлов...")
            .unwrap(),
    );
    count_pb.enable_steady_tick(Duration::from_millis(100));

    let files = files_from(directory)?;

    count_pb.finish_with_message(format!("завершен, файлов: {}", files.len()));

    if files.is_empty() {
        return Err(eyre!(
            "Нет доступных файлов для обработки в директории: {}",
            directory.display()
        ));
    }

    let current_file_pb = mp.add(ProgressBar::new(0));
    current_file_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );

    let pb = mp.add(ProgressBar::new(files.len() as u64));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.green}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=> "),
    );

    let existing_files = Arc::new(Mutex::new(HashSet::with_capacity(files.len())));
    let output_dir = Arc::new(output_dir);

    let chunk_size = (files.len() / rayon::current_num_threads()).max(1);

    files.par_chunks(chunk_size).for_each(|chunk| {
        let mut local_existing = HashSet::new();

        for entry in chunk {
            let file_name = entry.file_name().to_string_lossy().to_string();
            current_file_pb.set_message(format!("Обработка: {}", file_name));

            let mut dest_path = PathBuf::from(&*output_dir).join(&file_name);
            let mut counter = 1;

            while local_existing.contains(dest_path.to_str().unwrap())
                || existing_files
                    .lock()
                    .unwrap()
                    .contains(dest_path.to_str().unwrap())
            {
                let new_name = if let Some(ext) = dest_path.extension() {
                    format!(
                        "{} ({}).{}",
                        dest_path.file_stem().unwrap().to_string_lossy(),
                        counter,
                        ext.to_string_lossy()
                    )
                } else {
                    format!("{} ({})", file_name, counter)
                };
                dest_path = PathBuf::from(&*output_dir).join(new_name);
                counter += 1;
            }

            let result = if move_files {
                fs::rename(entry.path(), &dest_path)
            } else {
                fs::copy(entry.path(), &dest_path).map(|_| ())
            };

            if let Err(e) = result {
                eprintln!(
                    "Ошибка {} файла {}: {}",
                    if move_files {
                        "перемещения"
                    } else {
                        "копирования"
                    },
                    entry.path().display(),
                    e
                );
            } else {
                local_existing.insert(dest_path.to_str().unwrap().to_string());
            }

            pb.inc(1);
        }

        let mut global_existing = existing_files.lock().unwrap();
        global_existing.extend(local_existing);
    });

    current_file_pb.finish_and_clear();
    pb.finish_with_message("Обработка файлов завершена");

    if existing_files.lock().unwrap().is_empty() {
        eprintln!("Ничего не сделано.");
    } else {
        println!("Все файлы уплощены в {}", output_dir);
    }
    Ok(())
}
