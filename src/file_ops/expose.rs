use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use crate::file_ops::files_from;
#[cfg(unix)]
use crate::platform::unix::{print_unix_warning, rename_hidden_files};
#[cfg(windows)]
use crate::platform::windows::remove_hidden_attribute;

pub fn process_expose(directory: &str, _force: bool) {
    let mp = MultiProgress::new();

    #[cfg(unix)]
    {
        if _force {
            let files = files_from(directory);

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
                    .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                    .unwrap()
                    .progress_chars("=> "),
            );

            let current_file_pb_clone = current_file_pb.clone();
            let pb_clone = pb.clone();
            let handle_pb_state = thread::spawn(move || {
                while !current_file_pb_clone.is_finished() && !pb_clone.is_finished() {
                    current_file_pb_clone.tick();
                    pb_clone.tick();
                    thread::sleep(Duration::from_millis(100));
                }
            });

            rename_hidden_files(directory);

            pb.finish_with_message("Операция завершена");
            current_file_pb.finish_and_clear();

            handle_pb_state.join().unwrap();
            println!("Раскрыто файлов: {}", files.len());
        } else {
            print_unix_warning(directory);
        }
    }

    #[cfg(windows)]
    {
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

        let processed_count = AtomicUsize::new(0); // Счетчик обработанных файлов

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
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=> "),
        );

        let current_file_pb_clone = current_file_pb.clone();
        let pb_clone = pb.clone();
        let handle_pb_state = thread::spawn(move || {
            while !current_file_pb_clone.is_finished() && !pb_clone.is_finished() {
                current_file_pb_clone.tick();
                pb_clone.tick();
                thread::sleep(Duration::from_millis(100));
            }
        });

        files.par_iter().for_each(|entry| {
            let path = entry.path().display().to_string();
            current_file_pb.set_message(format!("Обработка: {}", entry.file_name().to_string_lossy().to_string()));

            if remove_hidden_attribute(&path) {
                processed_count.fetch_add(1, Ordering::SeqCst);
            }
            pb.inc(1);
        });

        pb.finish_with_message("Операция завершена");
        current_file_pb.finish_and_clear();

        handle_pb_state.join().unwrap();
        println!("Раскрыто файлов: {}", processed_count.load(Ordering::SeqCst)); // Выводим количество обработанных файлов
    }
}
