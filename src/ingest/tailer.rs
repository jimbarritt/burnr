use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// Follows a growing file, returning newly appended, complete lines on each
/// `poll`. A trailing line with no newline yet is buffered until it
/// completes.
pub struct Tailer {
    file: File,
    leftover: String,
}

impl Tailer {
    pub fn open_at_end(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        file.seek(SeekFrom::End(0))?;
        Ok(Self {
            file,
            leftover: String::new(),
        })
    }

    pub fn open_from_start(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self {
            file,
            leftover: String::new(),
        })
    }

    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        let mut chunk = String::new();
        self.file.read_to_string(&mut chunk)?;
        if chunk.is_empty() {
            return Ok(Vec::new());
        }
        self.leftover.push_str(&chunk);

        let mut lines = Vec::new();
        while let Some(newline_idx) = self.leftover.find('\n') {
            let line: String = self.leftover.drain(..=newline_idx).collect();
            let line = line.trim_end_matches('\n');
            if !line.is_empty() {
                lines.push(line.to_string());
            }
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "burnr-tailer-test-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn reads_lines_appended_after_open() {
        let path = temp_path("append");
        File::create(&path).unwrap();

        let mut tailer = Tailer::open_at_end(&path).unwrap();
        assert_eq!(tailer.poll().unwrap(), Vec::<String>::new());

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(file, "line one").unwrap();
        writeln!(file, "line two").unwrap();

        assert_eq!(tailer.poll().unwrap(), vec!["line one", "line two"]);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn buffers_partial_trailing_line_until_complete() {
        let path = temp_path("partial");
        File::create(&path).unwrap();

        let mut tailer = Tailer::open_at_end(&path).unwrap();

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        write!(file, "half a line").unwrap();
        assert_eq!(tailer.poll().unwrap(), Vec::<String>::new());

        writeln!(file, " and the rest").unwrap();
        assert_eq!(tailer.poll().unwrap(), vec!["half a line and the rest"]);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn open_from_start_reads_existing_content() {
        let path = temp_path("from-start");
        std::fs::write(&path, "already here\n").unwrap();

        let mut tailer = Tailer::open_from_start(&path).unwrap();
        assert_eq!(tailer.poll().unwrap(), vec!["already here"]);

        std::fs::remove_file(&path).unwrap();
    }
}
