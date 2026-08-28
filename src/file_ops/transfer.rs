use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const NAME_LOCK_STRIPES: usize = 64;

pub fn name_stripe(name: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish() as usize % NAME_LOCK_STRIPES
}

pub fn modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub fn transfer(src: &Path, dest: &Path, move_files: bool) -> io::Result<()> {
    if move_files {
        fs::rename(src, dest)
    } else {
        fs::copy(src, dest).map(|_| ())
    }
}

pub fn replace_newest_transfer(
    src: &Path,
    dest: &Path,
    move_files: bool,
) -> io::Result<Option<PathBuf>> {
    let dest_exists = dest.try_exists()?;
    if dest_exists {
        let keep_source = match (modified_time(src), modified_time(dest)) {
            (Some(src_time), Some(dest_time)) => src_time > dest_time,
            _ => false,
        };
        if !keep_source {
            return Ok(None);
        }
        fs::remove_file(dest)?;
    }
    transfer(src, dest, move_files)?;
    Ok(Some(dest.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dir_tools_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("создание временной директории");
        dir
    }

    fn write(path: &Path, content: &str) {
        fs::write(path, content).expect("запись тестового файла");
    }

    #[test]
    fn stripe_is_deterministic_and_within_bounds() {
        assert_eq!(name_stripe("a.txt"), name_stripe("a.txt"));
        for i in 0..200 {
            assert!(name_stripe(&format!("name{i}")) < NAME_LOCK_STRIPES);
        }
    }

    #[test]
    fn copies_when_dest_missing() {
        let dir = temp_dir("missing");
        let (src, dest) = (dir.join("src.txt"), dir.join("dest.txt"));
        write(&src, "data");

        let result = replace_newest_transfer(&src, &dest, false).unwrap();

        assert_eq!(result, Some(dest.clone()));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "data");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn skips_when_dest_newer() {
        let dir = temp_dir("skip_older");
        let (src, dest) = (dir.join("src.txt"), dir.join("dest.txt"));
        write(&src, "old");
        thread::sleep(Duration::from_millis(50));
        write(&dest, "new");

        let result = replace_newest_transfer(&src, &dest, false).unwrap();

        assert_eq!(result, None);
        assert_eq!(fs::read_to_string(&dest).unwrap(), "new");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn replaces_when_source_newer() {
        let dir = temp_dir("replace_newer");
        let (src, dest) = (dir.join("src.txt"), dir.join("dest.txt"));
        write(&dest, "old");
        thread::sleep(Duration::from_millis(50));
        write(&src, "fresh");

        let result = replace_newest_transfer(&src, &dest, false).unwrap();

        assert_eq!(result, Some(dest.clone()));
        assert_eq!(fs::read_to_string(&dest).unwrap(), "fresh");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn move_mode_removes_source() {
        let dir = temp_dir("move");
        let (src, dest) = (dir.join("src.txt"), dir.join("dest.txt"));
        write(&src, "data");

        let result = replace_newest_transfer(&src, &dest, true).unwrap();

        assert_eq!(result, Some(dest.clone()));
        assert!(!src.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "data");
        fs::remove_dir_all(&dir).unwrap();
    }
}
