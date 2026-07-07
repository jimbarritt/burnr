use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::{EventDeduper, Tailer, TokenBurnEvent, find_latest_session, parse_line};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Spawns a background thread that finds the most recent session transcript
/// in `log_dir`, tails it from its current end, and sends a `TokenBurnEvent`
/// for each new, non-duplicate usage snapshot. If no session file exists yet
/// it retries until one appears, so it's safe to call before Claude Code has
/// started writing to this project's log directory. If a newer session file
/// appears later (a fresh session started), the watcher rotates to it and
/// tails the new file from the start so its earliest turns aren't lost.
pub fn spawn_watcher(log_dir: PathBuf) -> mpsc::Receiver<TokenBurnEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let session_path = loop {
            if let Ok(Some(path)) = find_latest_session(&log_dir) {
                break path;
            }
            thread::sleep(POLL_INTERVAL);
        };
        let Ok(tailer) = Tailer::open_at_end(&session_path) else {
            return;
        };
        watch_loop(Some(&log_dir), session_path, tailer, &tx);
    });
    rx
}

/// Like `spawn_watcher`, but pinned to one specific transcript: no session
/// discovery and no rotation to newer files. The file must already exist —
/// the caller validates the path so the error surfaces before the terminal
/// opens.
pub fn spawn_session_watcher(session_path: PathBuf) -> io::Result<mpsc::Receiver<TokenBurnEvent>> {
    let tailer = Tailer::open_at_end(&session_path)?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || watch_loop(None, session_path, tailer, &tx));
    Ok(rx)
}

fn watch_loop(
    rotate_within: Option<&std::path::Path>,
    mut session_path: PathBuf,
    mut tailer: Tailer,
    tx: &mpsc::Sender<TokenBurnEvent>,
) {
    let mut deduper = EventDeduper::new();
    loop {
        // A failed rotation (e.g. the new file vanished between listing and
        // opening) just keeps tailing the current session; a poll error on
        // the current file ends the watcher — the receiver sees a closed
        // channel and the fire simply stays idle.
        if let Some(log_dir) = rotate_within
            && let Ok(Some(latest)) = find_latest_session(log_dir)
            && latest != session_path
            && let Ok(new_tailer) = Tailer::open_from_start(&latest)
        {
            session_path = latest;
            tailer = new_tailer;
            deduper = EventDeduper::new();
        }
        let Ok(lines) = tailer.poll() else { return };
        for line in lines {
            let Some(event) = parse_line(&line).and_then(|event| deduper.dedupe(event)) else {
                continue;
            };
            if tx.send(event).is_err() {
                return;
            }
        }
        thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    const ASSISTANT_LINE: &str = r#"{"type":"assistant","message":{"id":"msg_1","usage":{"input_tokens":2,"output_tokens":114,"cache_read_input_tokens":56249,"cache_creation_input_tokens":279}}}"#;

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "burnr-watcher-test-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn watcher_emits_event_for_line_appended_to_latest_session() {
        let dir = tempdir("basic");
        let session_path = dir.join("session.jsonl");
        fs::write(&session_path, "").unwrap();

        let rx = spawn_watcher(dir.clone());

        // `open_at_end` seeks to whatever the file's end is at open time, so
        // a write landing before the watcher thread gets scheduled would be
        // skipped (by design — this is a live monitor, not a replay). Give
        // it a couple of poll intervals' head start to open the file first.
        thread::sleep(POLL_INTERVAL * 2);

        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&session_path)
            .unwrap();
        writeln!(file, "{ASSISTANT_LINE}").unwrap();

        let event = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("expected an event");
        assert_eq!(event.message_id, "msg_1");
        assert_eq!(event.total_tokens(), 2 + 114 + 56249 + 279);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn watcher_waits_for_session_file_to_appear() {
        let dir = tempdir("waits");

        let rx = spawn_watcher(dir.clone());

        // No session file yet — the watcher should retry rather than give up.
        thread::sleep(POLL_INTERVAL * 2);

        let session_path = dir.join("session.jsonl");
        fs::write(&session_path, "").unwrap();
        // Same head start as above: let the watcher notice and open the
        // now-existing file before we write to it.
        thread::sleep(POLL_INTERVAL * 2);
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&session_path)
            .unwrap();
        writeln!(file, "{ASSISTANT_LINE}").unwrap();

        let event = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("expected an event");
        assert_eq!(event.message_id, "msg_1");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn session_watcher_stays_pinned_to_its_file() {
        let dir = tempdir("pinned");
        let pinned = dir.join("pinned.jsonl");
        fs::write(&pinned, "").unwrap();

        let rx = spawn_session_watcher(pinned.clone()).unwrap();
        thread::sleep(POLL_INTERVAL * 2);

        // A newer file appearing must NOT steal the watcher when pinned.
        let newer = dir.join("newer.jsonl");
        fs::write(
            &newer,
            format!("{}\n", ASSISTANT_LINE.replace("msg_1", "msg_other")),
        )
        .unwrap();
        thread::sleep(POLL_INTERVAL * 2);

        let mut file = fs::OpenOptions::new().append(true).open(&pinned).unwrap();
        writeln!(file, "{ASSISTANT_LINE}").unwrap();

        let event = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("expected an event from the pinned session");
        assert_eq!(event.message_id, "msg_1");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn session_watcher_errors_on_missing_file() {
        assert!(spawn_session_watcher(PathBuf::from("/nonexistent/nope.jsonl")).is_err());
    }

    #[test]
    fn watcher_rotates_to_a_newer_session_file() {
        let dir = tempdir("rotates");
        let old_session = dir.join("old-session.jsonl");
        fs::write(&old_session, "").unwrap();

        let rx = spawn_watcher(dir.clone());
        thread::sleep(POLL_INTERVAL * 2);

        // A fresh session starts: a new transcript file appears with a newer
        // mtime, already containing a turn. Rotation opens it from the
        // start, so that turn must come through even though it was written
        // before the watcher switched over.
        let new_session = dir.join("new-session.jsonl");
        let new_line = ASSISTANT_LINE.replace("msg_1", "msg_2");
        fs::write(&new_session, format!("{new_line}\n")).unwrap();

        let event = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("expected an event from the rotated-to session");
        assert_eq!(event.message_id, "msg_2");

        fs::remove_dir_all(&dir).unwrap();
    }
}
