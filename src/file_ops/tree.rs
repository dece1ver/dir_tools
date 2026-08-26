use crate::file_ops::{create_progressbar, CustomStyle};
use eyre::Result;
use std::path::Path;
use walkdir::WalkDir;

pub fn process_tree(
    directory: &Path,
    show_content: bool,
    max_depth: usize,
    full_content: bool,
    show_hidden: bool,
) -> Result<()> {
    println!("{}", directory.display());

    let pb = create_progressbar(
        "[{elapsed_precise}] {spinner:.green} {msg}",
        CustomStyle::Spinner,
        0,
    );
    pb.set_message("Подсчет файлов...");

    let walker = WalkDir::new(directory).min_depth(1).follow_links(true);

    let walker = if max_depth > 0 {
        walker.max_depth(max_depth)
    } else {
        walker
    };

    let entries: Vec<_> = walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            if show_hidden {
                true
            } else {
                !e.file_name()
                    .to_str()
                    .map(|s| s.starts_with("."))
                    .unwrap_or(false)
            }
        })
        .collect();

    pb.finish_and_clear();

    let config = TreeConfig {
        entries: &entries,
        show_content,
        full_content,
        max_depth,
    };

    print_tree(directory, "", true, 1, &config)?;
    Ok(())
}

struct TreeConfig<'a> {
    entries: &'a [walkdir::DirEntry],
    show_content: bool,
    full_content: bool,
    max_depth: usize,
}

fn print_tree(
    path: &Path,
    prefix: &str,
    is_last: bool,
    current_depth: usize,
    config: &TreeConfig<'_>,
) -> Result<()> {
    if config.max_depth > 0 && current_depth > config.max_depth {
        return Ok(());
    }

    let entries: Vec<_> = config
        .entries
        .iter()
        .filter(|e| e.path().parent() == Some(path))
        .collect();

    for (index, entry) in entries.iter().enumerate() {
        let is_last_entry = index == entries.len() - 1;
        let entry_path = entry.path();
        let file_name = entry.file_name();

        let (current_prefix, next_prefix) = if is_last {
            ("└── ", "    ")
        } else {
            ("├── ", "│   ")
        };

        print!(
            "{}{}{}",
            prefix,
            current_prefix,
            file_name.to_string_lossy()
        );

        if entry.file_type().is_file() {
            println!();
            if config.show_content
                && let Ok(content) = std::fs::read_to_string(entry_path)
            {
                let lines: Vec<_> = content.lines().collect();
                let lines_to_show = if config.full_content {
                    lines.as_slice()
                } else {
                    &lines[..lines.len().min(5)]
                };

                for line in lines_to_show {
                    println!("{}{}│ {}", prefix, next_prefix, line);
                }

                if !config.full_content && lines.len() > 5 {
                    println!("{}{}│ ...", prefix, next_prefix);
                }
            }
        } else {
            println!("/");
            print_tree(
                entry_path,
                &format!("{}{}", prefix, if is_last { next_prefix } else { "│   " }),
                is_last_entry,
                current_depth + 1,
                config,
            )?;
        }
    }
    Ok(())
}
