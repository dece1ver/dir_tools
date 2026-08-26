use eyre::{Result, eyre};
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use indicatif::MultiProgress;
use rayon::prelude::*;

use crate::TICK_DURATION;
use crate::file_ops::files_from;

use super::{CustomStyle, create_progressbar};

const NAME_LOCK_STRIPES: usize = 64;

fn name_stripe(name: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish() as usize % NAME_LOCK_STRIPES
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn transfer(src: &Path, dest: &Path, move_files: bool) -> io::Result<()> {
    if move_files {
        fs::rename(src, dest)
    } else {
        fs::copy(src, dest).map(|_| ())
    }
}

fn replace_newest_transfer(
    src: &Path,
    dest: &Path,
    move_files: bool,
) -> io::Result<Option<PathBuf>> {
    let dest_exists = dest.try_exists()?;
    if dest_exists {
        let keep_source = match (modified_time(src), modified_time(dest)) {
            (Some(src_time), Some(dest_time)) => src_time > dest_time,
            _ => false,
        };
        if !keep_source {
            return Ok(None);
        }
        fs::remove_file(dest)?;
    }
    transfer(src, dest, move_files)?;
    Ok(Some(dest.to_path_buf()))
}

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
        "{spinner:.green} [{elapsed_precise}] Подсчет файлов...",
        CustomStyle::Spinner,
        0,
    ));
    count_pb.enable_steady_tick(TICK_DURATION);

    let files = files_from(directory)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dir_tools_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("создание временной директории");
        dir
    }

    fn write(path: &Path, content: &str) {
        fs::write(path, content).expect("запись тестового файла");
    }

    #[test]
    fn stripe_is_deterministic_and_within_bounds() {
        assert_eq!(name_stripe("a.txt"), name_stripe("a.txt"));
        for i in 0..200 {
            assert!(name_stripe(&format!("name{i}")) < NAME_LOCK_STRIPES);
        }
    }

    #[test]
    fn copies_when_dest_missing() {
        let dir = temp_dir("missing");
        let (src, dest) = (dir.join("src.txt"), dir.join("dest.txt"));
        write(&src, "data");

        let result = replace_newest_transfer(&src, &dest, false).unwrap();

        assert_eq!(result, Some(dest.clone()));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "data");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn skips_when_dest_newer() {
        let dir = temp_dir("skip_older");
        let (src, dest) = (dir.join("src.txt"), dir.join("dest.txt"));
        write(&src, "old");
        thread::sleep(Duration::from_millis(50));
        write(&dest, "new");

        let result = replace_newest_transfer(&src, &dest, false).unwrap();

        assert_eq!(result, None);
        assert_eq!(fs::read_to_string(&dest).unwrap(), "new");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn replaces_when_source_newer() {
        let dir = temp_dir("replace_newer");
        let (src, dest) = (dir.join("src.txt"), dir.join("dest.txt"));
        write(&dest, "old");
        thread::sleep(Duration::from_millis(50));
        write(&src, "fresh");

        let result = replace_newest_transfer(&src, &dest, false).unwrap();

        assert_eq!(result, Some(dest.clone()));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "fresh");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn move_mode_removes_source() {
        let dir = temp_dir("move");
        let (src, dest) = (dir.join("src.txt"), dir.join("dest.txt"));
        write(&src, "data");

        let result = replace_newest_transfer(&src, &dest, true).unwrap();

        assert_eq!(result, Some(dest.clone()));
        assert!(!src.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "data");
        fs::remove_dir_all(&dir).unwrap();
    }
}
