use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn slugify_cwd(cwd: &Path) -> String {
    cwd.to_string_lossy().replace('/', "-")
}

pub fn claude_projects_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join("projects"))
}

pub fn project_log_dir(cwd: &Path) -> Option<PathBuf> {
    claude_projects_dir().map(|base| base.join(slugify_cwd(cwd)))
}

pub fn find_latest_session(dir: &Path) -> io::Result<Option<PathBuf>> {
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let modified = entry.metadata()?.modified()?;
        if latest.as_ref().is_none_or(|(seen, _)| modified > *seen) {
            latest = Some((modified, path));
        }
    }
    Ok(latest.map(|(_, path)| path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_replaces_path_separators() {
        let cwd = Path::new("/Users/jmdb/Code/github/jimbarritt/burnr");
        assert_eq!(slugify_cwd(cwd), "-Users-jmdb-Code-github-jimbarritt-burnr");
    }

    #[test]
    fn find_latest_session_picks_newest_jsonl() {
        let dir = tempdir();
        let older = dir.join("older.jsonl");
        let newer = dir.join("newer.jsonl");
        fs::write(&older, "{}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&newer, "{}").unwrap();
        fs::write(dir.join("ignored.txt"), "not jsonl").unwrap();

        let found = find_latest_session(&dir).unwrap();
        assert_eq!(found, Some(newer));

        fs::remove_dir_all(&dir).unwrap();
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("burnr-test-{}", uuid_like()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uuid_like() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
