use eyre::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use crate::file_ops::files_from;
#[cfg(unix)]
use crate::platform::unix::{print_unix_warning, rename_hidden_files};
#[cfg(windows)]
use crate::platform::windows::remove_hidden_attribute;

const TICK_DURATION: Duration = Duration::from_millis(100);
const PROGRESS_CHARS: &str = "=> ";

pub fn process_expose(directory: impl AsRef<Path>, _force: bool) -> Result<()> {
    let directory = directory.as_ref();
    let mp = MultiProgress::new();

    #[cfg(unix)]
    {
        if _force {
            let files = files_from(directory);
            let file_count = files.len() as u64;

            let current_file_pb = mp.add(ProgressBar::new(0));
            current_file_pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .expect("Failed to create spinner style"),
            );

            let pb = mp.add(ProgressBar::new(file_count));
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                    .expect("Failed to create progress bar style")
                    .progress_chars(PROGRESS_CHARS),
            );

            let current_file_pb_clone = current_file_pb.clone();
            let pb_clone = pb.clone();

            let handle_pb_state = thread::spawn(move || {
                while !current_file_pb_clone.is_finished() && !pb_clone.is_finished() {
                    current_file_pb_clone.tick();
                    pb_clone.tick();
                    thread::sleep(TICK_DURATION);
                }
            });

            rename_hidden_files(directory);

            pb.finish_with_message("Операция завершена");
            current_file_pb.finish_and_clear();

            handle_pb_state
                .join()
                .expect("Progress bar thread panicked");
            println!("Раскрыто файлов: {file_count}");
        } else {
            print_unix_warning(directory);
        }
    }

    #[cfg(windows)]
    {
        let count_pb = mp.add(ProgressBar::new_spinner());
        count_pb.set_style(
            ProgressStyle::default_spinner().template("{spinner:.green} Подсчет файлов...")?,
        );
        count_pb.enable_steady_tick(TICK_DURATION);

        let files = files_from(directory)?;
        let file_count = files.len();
        count_pb.finish_with_message(format!("завершен, файлов: {file_count}"));

        let processed_count = AtomicUsize::new(0);

        let current_file_pb = mp.add(ProgressBar::new(0));
        current_file_pb
            .set_style(ProgressStyle::default_spinner().template("{spinner:.green} {msg}")?);

        let pb = mp.add(ProgressBar::new(file_count as u64));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars(PROGRESS_CHARS),
        );

        let current_file_pb_clone = current_file_pb.clone();
        let pb_clone = pb.clone();

        let handle_pb_state = thread::spawn(move || {
            while !current_file_pb_clone.is_finished() && !pb_clone.is_finished() {
                current_file_pb_clone.tick();
                pb_clone.tick();
                thread::sleep(TICK_DURATION);
            }
        });

        files.par_iter().for_each(|entry| {
            let path = entry.path().display().to_string();
            let filename = entry.file_name().to_string_lossy();

            current_file_pb.set_message(format!("Обработка: {filename}"));

            if remove_hidden_attribute(&path) {
                processed_count.fetch_add(1, Ordering::SeqCst);
            }
            pb.inc(1);
        });

        pb.finish_with_message("Операция завершена");
        current_file_pb.finish_and_clear();

        handle_pb_state.join().unwrap();

        let processed = processed_count.load(Ordering::SeqCst);
        println!("Раскрыто файлов: {processed}");
    }
    Ok(())
}
