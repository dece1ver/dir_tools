use eyre::{eyre, Result};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::TICK_DURATION;

use super::transfer::{name_stripe, replace_newest_transfer, NAME_LOCK_STRIPES};
use super::{create_progressbar, format_live_count, CustomStyle};

struct MergePlan {
    roots: HashMap<String, PathBuf>,
    root_patterns: Vec<Regex>,
}

fn glob_to_regex(pattern: &str) -> Regex {
    let mut re = String::from("^");
    for c in pattern.chars() {
        match c {
            '*' => re.push_str("[^/]*"),
            '?' => re.push_str("[^/]"),
            '.' | '+' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '\\' => {
                re.push('\\');
                re.push(c);
            }
            c => re.push(c),
        }
    }
    re.push('$');
    let with_flags = if cfg!(windows) {
        format!("(?i){re}")
    } else {
        re
    };
    Regex::new(&with_flags).unwrap_or_else(|_| Regex::new("$^").unwrap())
}

fn build_merge_plan(directory: &Path, from: Option<&[String]>) -> Result<MergePlan> {
    match from {
        Some(patterns) if !patterns.is_empty() => {
            let root_patterns = patterns.iter().map(|p| glob_to_regex(p)).collect();
            Ok(MergePlan {
                roots: HashMap::new(),
                root_patterns,
            })
        }
        _ => {
            let mut roots = HashMap::new();
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let rel = entry
                        .path()
                        .strip_prefix(directory)
                        .map(|p| p.to_path_buf())
                        .unwrap_or_default();
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if !rel_str.is_empty() {
                        roots.insert(rel_str.clone(), PathBuf::from(&rel_str));
                    }
                }
            }
            Ok(MergePlan {
                roots,
                root_patterns: Vec::new(),
            })
        }
    }
}

fn find_root<'a>(rel: &str, roots: &'a HashMap<String, PathBuf>) -> Option<&'a PathBuf> {
    let mut best: Option<&PathBuf> = None;
    let mut path = PathBuf::new();
    for component in Path::new(rel).components() {
        path.push(component);
        let key = path.to_string_lossy().replace('\\', "/");
        if let Some(root) = roots.get(&key) {
            best = Some(root);
        }
    }
    best
}

fn walk_and_collect(
    directory: &Path,
    plan: &mut MergePlan,
    output_dir: &Path,
    pb: &ProgressBar,
) -> Result<Vec<(walkdir::DirEntry, PathBuf)>> {
    let mut targets = Vec::new();

    for entry in WalkDir::new(directory)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = match entry.path().strip_prefix(directory) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => continue,
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str.is_empty() {
            continue;
        }

        if entry.file_type().is_dir() {
            if !plan.root_patterns.is_empty()
                && plan
                    .root_patterns
                    .iter()
                    .any(|re| re.is_match(&rel_str))
            {
                plan.roots
                    .insert(rel_str.clone(), PathBuf::from(&rel_str));
            }
            if let Some(root) = find_root(&rel_str, &plan.roots) {
                let root_len = root.to_string_lossy().len();
                let dest_rel = PathBuf::from(rel_str[root_len..].trim_start_matches('/'));
                fs::create_dir_all(output_dir.join(&dest_rel))?;
            }
        } else if let Some(root) = find_root(&rel_str, &plan.roots) {
            pb.inc(1);
            let root_len = root.to_string_lossy().len();
            let dest_rel = PathBuf::from(rel_str[root_len..].trim_start_matches('/'));
            pb.set_message(format_live_count(&rel_str, pb.position()));
            targets.push((entry, dest_rel));
        }
    }

    Ok(targets)
}

pub fn process_merge(
    directory: impl AsRef<Path>,
    output: Option<&PathBuf>,
    from: Option<&[String]>,
    move_files: bool,
) -> Result<()> {
    let directory = directory.as_ref();

    let output_dir = match output {
        Some(path) => path.clone(),
        None => {
            let mut output_path = std::env::current_exe()?;
            output_path.pop();
            output_path.push("merged_files");
            output_path
        }
    };

    fs::create_dir_all(&output_dir)?;

    let mp = MultiProgress::new();
    let count_pb = mp.add(create_progressbar(
        "{spinner:.green} [{elapsed_precise}] {msg}",
        CustomStyle::Spinner,
        0,
    ));
    count_pb.enable_steady_tick(TICK_DURATION);
    count_pb.set_message("Поиск снимков...");

    let mut plan = build_merge_plan(directory, from)?;
    let targets = walk_and_collect(directory, &mut plan, &output_dir, &count_pb)?;

    if plan.roots.is_empty() {
        return Err(eyre!(
            "Не найдено ни одного снимка для слияния в директории: {}",
            directory.display()
        ));
    }

    println!("Найдено {} снимков для слияния", plan.roots.len());

    let errors = Arc::new(Mutex::new(Vec::new()));
    count_pb.finish_and_clear();

    if targets.is_empty() {
        return Err(eyre!(
            "Нет доступных файлов для обработки в директории: {}",
            directory.display()
        ));
    }

    let current_file_pb = mp.add(create_progressbar(
        "{spinner:.green} {msg}",
        CustomStyle::Spinner,
        0,
    ));

    let pb = mp.add(create_progressbar(
        "[{elapsed_precise}] {bar:40.green} {pos}/{len} ({eta})",
        CustomStyle::Bar,
        targets.len(),
    ));

    let existing_files = Arc::new(Mutex::new(HashSet::with_capacity(targets.len())));
    let skipped_files = Arc::new(Mutex::new(Vec::new()));
    let output_dir = Arc::new(output_dir);
    let name_locks: Vec<Mutex<()>> = (0..NAME_LOCK_STRIPES).map(|_| Mutex::new(())).collect();

    let chunk_size = (targets.len() / rayon::current_num_threads()).max(1);

    targets.par_chunks(chunk_size).for_each(|chunk| {
        let mut local_existing = HashSet::new();

        for (entry, dest_rel) in chunk {
            if dest_rel.as_os_str().is_empty() {
                pb.inc(1);
                continue;
            }
            current_file_pb.set_message(format!("Обработка: {}", dest_rel.display()));

            let stripe_key = dest_rel.to_string_lossy().into_owned();
            let _guard = name_locks[name_stripe(&stripe_key)]
                .lock()
                .unwrap_or_else(PoisonError::into_inner);

            let dest_path = PathBuf::from(&*output_dir).join(dest_rel);

            let result = replace_newest_transfer(entry.path(), &dest_path, move_files);

            match result {
                Ok(Some(_)) => {
                    local_existing.insert(dest_path.to_string_lossy().to_string());
                }
                Ok(None) => {
                    skipped_files
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(entry.path().display().to_string());
                }
                Err(e) => {
                    let mut errors = errors.lock().unwrap_or_else(PoisonError::into_inner);
                    errors.push(format!(
                        "Ошибка {} файла {}: {}",
                        if move_files {
                            "перемещения"
                        } else {
                            "копирования"
                        },
                        entry.path().display(),
                        e
                    ));
                }
            }

            pb.inc(1);
        }

        let mut global_existing = existing_files
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        global_existing.extend(local_existing);
    });

    current_file_pb.finish_and_clear();
    pb.finish_with_message("Обработка файлов завершена");

    let skipped = skipped_files.lock().unwrap_or_else(PoisonError::into_inner);
    if existing_files
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_empty()
        && skipped.is_empty()
    {
        eprintln!("Ничего не сделано.");
    } else {
        println!("Директории слиты в {}", output_dir.display());
        if !skipped.is_empty() {
            for file in skipped.iter() {
                eprintln!("Пропущен более старый файл: {file}");
            }
        }
    }
    let errors = errors.lock().unwrap_or_else(PoisonError::into_inner);
    if !errors.is_empty() {
        for err in errors.iter() {
            eprintln!("{err}")
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("dir_tools_merge_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("создание временной директории");
        dir
    }

    fn write(path: &Path, content: &str) {
        fs::write(path, content).expect("запись тестового файла");
    }

    fn scenario(root: &Path) {
        for snap in ["a", "b"] {
            fs::create_dir_all(root.join(snap).join("sub")).unwrap();
            fs::create_dir_all(root.join(snap).join("empty")).unwrap();
        }
        write(&root.join("a/readme.md"), "old");
        thread::sleep(Duration::from_millis(50));
        write(&root.join("b/readme.md"), "new");
        write(&root.join("a/sub/config.txt"), "sub_old");
        thread::sleep(Duration::from_millis(50));
        write(&root.join("b/sub/config.txt"), "sub_new");
        write(&root.join("a/only_a.txt"), "only_a");
    }

    fn assert_merged(out: &Path) {
        assert_eq!(fs::read_to_string(out.join("readme.md")).unwrap(), "new");
        assert_eq!(
            fs::read_to_string(out.join("sub/config.txt")).unwrap(),
            "sub_new"
        );
        assert_eq!(
            fs::read_to_string(out.join("only_a.txt")).unwrap(),
            "only_a"
        );
        assert!(out.join("empty").is_dir());
        assert!(!out.join("a").exists());
        assert!(!out.join("b").exists());
    }

    #[test]
    fn merge_default_uses_direct_children() {
        let root = temp_dir("default");
        scenario(&root);
        let out = root.join("out");

        process_merge(&root, Some(&out), None, false).unwrap();

        assert_merged(&out);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn merge_with_from_glob() {
        let root = temp_dir("glob");
        scenario(&root);
        let out = root.join("out");

        process_merge(
            &root,
            Some(&out),
            Some(&["a".to_string(), "b".to_string()]),
            false,
        )
        .unwrap();

        assert_merged(&out);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn merge_unmatched_from_warns_and_continues() {
        let root = temp_dir("unmatched");
        scenario(&root);
        let out = root.join("out");

        assert!(
            process_merge(&root, Some(&out), Some(&["nope*".to_string()]), false).is_err()
        );

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn glob_star_suffix_matches_end() {
        let re = glob_to_regex("*snap");
        assert!(re.is_match("mysnap"));
        assert!(re.is_match("snap"));
        assert!(!re.is_match("snapA"));
        assert!(!re.is_match("snap/sub"));
    }

    #[test]
    fn glob_s_p_anchors_start_and_end() {
        let re = glob_to_regex("s*p");
        assert!(re.is_match("snap"));
        assert!(re.is_match("sp"));
        assert!(!re.is_match("mysnap"));
        assert!(!re.is_match("snapshot"));
    }

    #[test]
    fn glob_prefix_star() {
        let re = glob_to_regex("snap*");
        assert!(re.is_match("snapA"));
        assert!(re.is_match("snapB"));
        assert!(!re.is_match("mysnap"));
    }

    #[test]
    fn glob_question_single_char() {
        let re = glob_to_regex("snap?");
        assert!(re.is_match("snapA"));
        assert!(!re.is_match("snap"));
        assert!(!re.is_match("snapAA"));
    }

    #[test]
    fn glob_star_does_not_cross_slash() {
        let re = glob_to_regex("*");
        assert!(re.is_match("sub"));
        assert!(re.is_match(""));
        assert!(!re.is_match("a/b"));
    }

    #[test]
    fn glob_literal_escapes_specials() {
        let re = glob_to_regex("s.p");
        assert!(re.is_match("s.p"));
        assert!(!re.is_match("sxp"));
    }

    #[test]
    fn glob_nested_path() {
        let re = glob_to_regex("snapA/sub");
        assert!(re.is_match("snapA/sub"));
        assert!(!re.is_match("snapA/sub/x"));
        assert!(!re.is_match("snapB/sub"));
    }

    #[test]
    fn find_root_matches_deepest() {
        let mut roots = HashMap::new();
        roots.insert("snapA".to_string(), PathBuf::from("snapA"));
        roots.insert("snapA/sub".to_string(), PathBuf::from("snapA/sub"));

        assert_eq!(
            find_root("snapA/f.txt", &roots),
            Some(&PathBuf::from("snapA"))
        );
        assert_eq!(
            find_root("snapA/sub/f.txt", &roots),
            Some(&PathBuf::from("snapA/sub"))
        );
        assert_eq!(find_root("snapB/f.txt", &roots), None);
    }

    #[test]
    fn build_merge_plan_default_finds_direct_children() {
        let base = temp_dir("plan_default");
        for d in ["snapA", "snapB", "mysnap", "snapA/sub"] {
            fs::create_dir_all(base.join(d)).unwrap();
        }

        let plan = build_merge_plan(&base, None).unwrap();
        let mut keys: Vec<&String> = plan.roots.keys().collect();
        keys.sort();
        assert_eq!(keys, vec!["mysnap", "snapA", "snapB"]);

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn build_merge_plan_glob_compiles_patterns() {
        let plan =
            build_merge_plan(Path::new("/tmp"), Some(&["snap*".to_string()])).unwrap();
        assert_eq!(plan.root_patterns.len(), 1);
        assert!(plan.roots.is_empty());
    }
}
