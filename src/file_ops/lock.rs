use crate::args::LockMode;
use crossterm::event::{self, Event, KeyCode};
use eyre::Result;
use fs2::FileExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;
use std::time::{Duration, Instant};
use std::{fs, thread};

fn format_duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;
    format!("{:02}:{:02}", minutes, remaining_seconds)
}

pub fn process_lock(path: impl AsRef<Path>, timer: &Option<u64>, mode: &LockMode) -> Result<()> {
    let path = path.as_ref();
    let mp = MultiProgress::new();

    let status_pb = mp.add(ProgressBar::new_spinner());
    status_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    status_pb.enable_steady_tick(Duration::from_millis(100));

    let progress_pb = mp.add(ProgressBar::new(100));
    progress_pb.set_style(
        ProgressStyle::default_bar()
            .template("[{bar:40.green}] {pos:>2}% | {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let file = match mode {
        LockMode::Read => fs::OpenOptions::new().read(true).open(path),
        LockMode::Write => fs::OpenOptions::new().write(true).open(path),
        LockMode::ReadWrite => fs::OpenOptions::new().read(true).write(true).open(path),
    }?;

    file.try_lock_exclusive()?;
    match timer {
        Some(t) => {
            let start = Instant::now();
            let duration = Duration::from_secs(*t);

            while start.elapsed() < duration {
                if event::poll(Duration::from_millis(0))? {
                    if let Event::Key(key) = event::read()? {
                        if key.code == KeyCode::Esc {
                            status_pb.finish_with_message(format!(
                                "Блокировка файла {} прервана пользователем",
                                path.display()
                            ));
                            progress_pb.finish_and_clear();
                            return Ok(());
                        }
                    }
                }

                let elapsed = start.elapsed();
                let remaining = duration.as_secs() - elapsed.as_secs();
                let percent = ((elapsed.as_secs_f64() / duration.as_secs_f64()) * 100.0) as u64;

                // Обновляем статус
                status_pb.set_message(format!(
                    "Блокировка файла: {} | Осталось: {}",
                    path.display(),
                    format_duration(remaining)
                ));

                // Обновляем прогресс-бар
                progress_pb.set_position(percent);
                progress_pb.set_message(format!(
                    "Прошло {} из {}",
                    format_duration(elapsed.as_secs()),
                    format_duration(duration.as_secs())
                ));

                thread::sleep(Duration::from_millis(100));
            }

            status_pb.finish_with_message(format!("Блокировка файла {} завершена", path.display()));
            progress_pb.finish_and_clear();
        }
        None => {
            status_pb.set_message(format!(
                "Бессрочная блокировка файла: {} (ESC для выхода)",
                path.display()
            ));

            loop {
                if event::poll(Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        if key.code == KeyCode::Esc {
                            status_pb.finish_with_message(format!(
                                "Блокировка файла {} прервана пользователем",
                                path.display()
                            ));
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
