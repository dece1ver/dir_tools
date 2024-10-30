use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Операция для выполнения
    #[command(subcommand)]
    pub operation: Operation,
}

#[derive(Subcommand, Debug)]
pub enum Operation {
    /// Раскрыть все файлы в указанном пути
    Expose {
        /// Директория для выполнения работы
        directory: String,
        /// Принудительное выполнение операции (вместо предупреждения)
        #[arg(short = 'f', long)]
        force: bool,
    },
    /// Упрощение структуры директорий
    Flatten {
        /// Директория для выполнения работы
        directory: String,
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
        directory: String,
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
        directory: String,
    },
    /// Поиск
    Find {
        /// Директория для выполнения работы
        directory: String,
        /// Что искать
        target: String,
        /// Режим поиска
        #[arg(short = 'm', long, default_value = "file-name")]
        mode: FindMode,
        /// Вывод (путь к файлу, если не указывать, то stdout)
        #[arg(short = 'o', long, default_value = "")]
        output: String,
    },
}

#[derive(Subcommand, Debug, Clone, ValueEnum)]
pub enum RenameTarget {
    Dirs,
    Files,
    Both,
}

#[derive(Subcommand, Debug, Clone, ValueEnum)]
pub enum FindMode {
    FileName,
    Content,
    Regexp,
    Gavriluk,
}
