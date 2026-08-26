use std::fs;

use walkdir::WalkDir;

pub fn print_unix_warning(directory: &str) {
    println!("Предупреждение: В Unix-подобных системах скрытые файлы начинаются с точки (.).");
    println!("Используйте стандартные средства, например:");
    println!(
        "find {} -name \".*\" -exec sh -c 'mv \"$1\" \"$(dirname \"$1\")/$(basename \"$1\" | sed \"s/^\\.//\")\"' sh {{}} \\;",
        directory
    );
}

pub fn rename_hidden_files(directory: &str) {
    for entry in WalkDir::new(directory).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => continue,
            };

            if file_name.starts_with('.') {
                let new_file_name = &file_name[1..];
                let new_path = path.with_file_name(new_file_name);

                if let Err(e) = fs::rename(path, &new_path) {
                    eprintln!("Ошибка переименования файла {}: {}", path.display(), e);
                }
            }
        }
    }
}
