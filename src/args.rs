use clap::{Parser, Subcommand};

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
}
