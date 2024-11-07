use clap::{Parser, Subcommand, ValueEnum};
use std::fmt::{self, Display};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub operation: Operation,
}

#[derive(Subcommand, Debug)]
pub enum Operation {
    /// Раскрыть все файлы в указанном пути, сохраняя структуру директорий
    Expose {
        /// Директория для выполнения работы
        directory: PathBuf,
        /// Принудительное выполнение операции, перезаписывая существующие файлы
        #[arg(short = 'f', long)]
        force: bool,
    },
    /// Упрощение структуры директорий, перемещая или копируя файлы
    Flatten {
        /// Директория для выполнения работы
        directory: PathBuf,
        /// Директория для сохранения файлов
        #[arg(short = 'o', long)]
        output: Option<String>,
        /// Перемещать файлы вместо копирования
        #[arg(short = 'm', long)]
        move_files: bool,
    },
    /// Заменить часть названия файлов или директорий
    Rename {
        /// Директория для выполнения работы
        directory: PathBuf,
        /// Тип обрабатываемых объектов
        #[arg(default_value = "both")]
        target_type: RenameTarget,
        /// Что заменить
        #[arg(short = 'f', long)]
        find: String,
        /// На что заменить
        #[arg(short = 'r', long)]
        replace: String,
    },
    /// Добавить название родительской директории к имени файла
    AddParentDir {
        /// Директория для выполнения работы
        directory: PathBuf,
        /// Разделитель между именем родительской директории и именем файла
        #[arg(short = 'd', long, default_value = " ")]
        delimiter: String,
    },
    /// Поиск файлов по различным критериям
    Find {
        /// Директория для выполнения поиска
        directory: PathBuf,
        /// Режим поиска
        #[arg(default_value = "file-name")]
        mode: FindMode,
        /// Шаблон для поиска
        #[arg(short = 'p', long)]
        pattern: Option<String>,
        /// Путь для сохранения результатов поиска
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
    },
    /// Блокировка файла на чтение и запись
    Lock {
        /// Путь к файлу
        path: PathBuf,
        /// Время блокировки в секундах (не указывать для бессрочной блокировки)
        timer: Option<u64>,
        /// Режим блокировки
        #[arg(default_value = "read-write")]
        mode: LockMode,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum RenameTarget {
    Dirs,
    Files,
    Both,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum FindMode {
    FileName,
    Content,
    Regexp,
    Gavriluk,
}

impl Display for FindMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FindMode::FileName => "имя файла",
            FindMode::Content => "содержимое",
            FindMode::Regexp => "регулярное выражение",
            FindMode::Gavriluk => "режим Гаврилюка",
        })
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum LockMode {
    Read,
    Write,
    ReadWrite,
}
