use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::file_ops::files_from;

pub fn process_flatten(directory: &str, output: &Option<String>, move_files: bool) {
    // Определяем директорию для вывода
    let output_dir = output.clone().unwrap_or_else(|| {
        let mut output_path = std::env::current_exe().unwrap();
        output_path.pop();
        output_path.push("flattened_files");
        output_path.to_str().unwrap().to_string()
    });

    if let Err(e) = fs::create_dir_all(&output_dir) {
        eprintln!("Ошибка создания директории {}: {}", output_dir, e);
        return;
    }

    let mp = MultiProgress::new();

    // Создаем спиннер для подсчета файлов
    let count_pb = mp.add(ProgressBar::new_spinner());
    count_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} Подсчет файлов...")
            .unwrap(),
    );
    count_pb.enable_steady_tick(Duration::from_millis(100));

    // Собираем список файлов для обработки
    let files = files_from(directory);

    count_pb.finish_and_clear();

    if files.is_empty() {
        eprintln!("Нет доступных файлов для обработки в директории: {}", directory);
        return;
    }

    // Создаем прогресс-бар для обработки текущего файла
    let current_file_pb = mp.add(ProgressBar::new(0));
    current_file_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );

    // Создаем прогресс-бар для общего прогресса
    let pb = mp.add(ProgressBar::new(files.len() as u64));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.green}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=> "),
    );

    // Поток для обновления состояния прогрессбаров
    let current_file_pb_clone = current_file_pb.clone();
    let pb_clone = pb.clone();
    let handle_pb_state = thread::spawn(move || {
        while !current_file_pb_clone.is_finished() && !pb_clone.is_finished() {
            current_file_pb_clone.tick();
            pb_clone.tick();
            thread::sleep(Duration::from_millis(100));
        }
    });

    let mut existing_files = HashSet::new();

    // Основной процесс обработки файлов
    for entry in files {
        let file_name = entry.file_name().to_string_lossy().to_string();
        current_file_pb.set_message(format!("Обработка: {}", file_name));

        // Определяем путь назначения
        let mut dest_path = PathBuf::from(&output_dir).join(&file_name);
        let mut counter = 1;

        // Проверка уникальности имени файла
        while existing_files.contains(dest_path.to_str().unwrap()) {
            let new_name = match dest_path.extension() {
                Some(ext) => format!(
                    "{} ({}).{}",
                    dest_path.file_stem().unwrap().to_string_lossy(),
                    counter,
                    ext.to_string_lossy()
                ),
                None => format!("{} ({})", file_name, counter),
            };
            dest_path = PathBuf::from(&output_dir).join(new_name);
            counter += 1;
        }

        // Перемещение или копирование файла
        let result = if move_files {
            fs::rename(entry.path(), &dest_path)
        } else {
            fs::copy(entry.path(), &dest_path).map(|_| ())
        };

        if let Err(e) = result {
            eprintln!(
                "Ошибка {} файла {}: {}",
                if move_files { "перемещения" } else { "копирования" },
                entry.path().display(),
                e
            );
        } else {
            existing_files.insert(dest_path.to_str().unwrap().to_string());
        }

        pb.inc(1); // Увеличиваем общий прогресс
    }

    current_file_pb.finish_and_clear();
    pb.finish_with_message("Обработка файлов завершена");

    // Завершаем потоки
    handle_pb_state.join().unwrap();

    // Проверяем, были ли файлы успешно обработаны
    if existing_files.is_empty() {
        eprintln!("Ничего не сделано.");
    } else {
        println!("Все файлы уплощены в {}", output_dir);
    }
}
