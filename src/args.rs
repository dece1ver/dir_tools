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
    /// Раскрыть все файлы в указанном пути
    Expose {
        /// Директория для выполнения работы
        directory: PathBuf,
        /// Принудительное выполнение операции
        #[arg(short = 'f', long)]
        force: bool,
    },
    /// Упрощение структуры директорий
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
    /// Заменить часть названия
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
    /// Добавить название родительской директории к файлам
    AFN {
        /// Директория для выполнения работы
        directory: PathBuf,
    },
    /// Поиск
    Find {
        /// Директория для выполнения работы
        directory: PathBuf,
        /// Режим поиска
        #[arg(default_value = "file-name")]
        mode: FindMode,
        /// Что искать
        #[arg(short = 'p', long)]
        pattern: Option<String>,
        /// Вывод (путь к файлу)
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
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
