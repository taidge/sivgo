//! Per-window crash recovery. An OS lock keeps live drafts out of recovery lists.

use std::{
    cmp::Reverse,
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

use serde::{Deserialize, Serialize};
use sivgo_dom::{parse_svg, serialize_svg, SvgDocument, SvgElement};

const MAX_DRAFT_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Draft {
    name: String,
    svg: String,
}

/// Owns the lock for one session. Dropping it leaves any written draft available
/// for recovery. Only an explicit clean transition removes a draft.
pub(super) struct Slot {
    path: PathBuf,
    lock: File,
}

impl Slot {
    fn new(folder: &Path) -> io::Result<Self> {
        fs::create_dir_all(folder)?;
        let lock = tempfile::Builder::new()
            .prefix("draft-")
            .suffix(".lock")
            .tempfile_in(folder)?;
        lock.as_file().lock()?;
        let (lock, path) = lock.keep().map_err(|error| error.error)?;
        Ok(Self {
            path: path.with_extension("json"),
            lock,
        })
    }

    fn claim(path: PathBuf) -> io::Result<Option<Self>> {
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))
        {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        match lock.try_lock() {
            Ok(()) if path.is_file() => Ok(Some(Self { path, lock })),
            Ok(()) | Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    fn write(&self, draft: &Draft) -> io::Result<()> {
        let data = serde_json::to_vec(draft).map_err(io::Error::other)?;
        if data.len() as u64 > MAX_DRAFT_BYTES {
            return Err(io::Error::other(
                "drawing exceeds the 128 MiB recovery limit",
            ));
        }
        sivgo_io::write_atomic(&self.path, |file| file.write_all(&data))
    }

    pub(super) fn remove(self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        drop(self.lock);
        let _ = fs::remove_file(self.path.with_extension("lock"));
        Ok(())
    }
}

pub(super) struct Recovered {
    pub name: String,
    pub document: SvgDocument,
    slot: Slot,
}

impl Recovered {
    pub(super) fn discard(self) -> io::Result<()> {
        self.slot.remove()
    }
}

#[derive(Default)]
pub(super) struct Recovery {
    slot: Option<Slot>,
    deadline: Option<Instant>,
    interval: Duration,
    last: Option<Draft>,
}

impl Recovery {
    pub(super) fn reset(&mut self) -> io::Result<()> {
        self.deadline = None;
        self.last = None;
        self.slot.take().map_or(Ok(()), Slot::remove)
    }

    pub(super) fn adopt(&mut self, recovered: Recovered) -> (String, SvgDocument) {
        debug_assert!(self.slot.is_none());
        self.slot = Some(recovered.slot);
        (recovered.name, recovered.document)
    }

    /// An idle window wakes only when it has dirty work with recovery enabled.
    pub(super) fn schedule(
        &mut self,
        now: Instant,
        enabled: bool,
        interval: Duration,
        dirty: bool,
        dragging: bool,
    ) -> Option<Instant> {
        if !enabled || !dirty {
            self.deadline = None;
            return None;
        }
        if self.interval != interval {
            self.interval = interval;
            self.deadline = None;
        }
        let due = *self.deadline.get_or_insert(now + interval);
        if due <= now && dragging {
            // Try again shortly without taking a snapshot mid-gesture.
            return Some(now + Duration::from_secs(1));
        }
        Some(due)
    }

    pub(super) fn checkpoint(
        &mut self,
        folder: &Path,
        now: Instant,
        name: &str,
        document: &SvgDocument,
    ) -> io::Result<Instant> {
        let next = now + self.interval;
        self.deadline = Some(next);
        let svg = serialize_svg(document);
        if self
            .last
            .as_ref()
            .is_some_and(|last| last.svg == svg && last.name == name)
        {
            return Ok(next);
        }
        if svg.len() as u64 > MAX_DRAFT_BYTES {
            return Err(io::Error::other(
                "drawing exceeds the 128 MiB recovery limit",
            ));
        }
        let draft = Draft {
            name: name.to_owned(),
            svg,
        };
        if self.slot.is_none() {
            self.slot = Some(Slot::new(folder)?);
        }
        self.slot
            .as_ref()
            .expect("allocated recovery slot")
            .write(&draft)?;
        self.last = Some(draft);
        Ok(next)
    }
}

/// Claim the newest readable abandoned draft. Keep damaged copies on disk for
/// manual repair and continue to older candidates when possible.
pub(super) fn newest(folder: &Path) -> io::Result<Option<Recovered>> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json")
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("draft-"))
            && entry.file_type()?.is_file()
        {
            candidates.push((
                entry
                    .metadata()?
                    .modified()
                    .unwrap_or(SystemTime::UNIX_EPOCH),
                path,
            ));
        }
    }
    candidates.sort_unstable_by_key(|(modified, _)| Reverse(*modified));
    let mut last_error = None;
    for (_, path) in candidates {
        match read_candidate(path) {
            Ok(Some(recovered)) => return Ok(Some(recovered)),
            Ok(None) => {}
            Err(error) => last_error = Some(error),
        }
    }
    last_error.map_or(Ok(None), Err)
}

fn read_candidate(path: PathBuf) -> io::Result<Option<Recovered>> {
    let Some(slot) = Slot::claim(path)? else {
        return Ok(None);
    };
    let mut data = Vec::new();
    File::open(&slot.path)?
        .take(MAX_DRAFT_BYTES + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_DRAFT_BYTES {
        return Err(io::Error::other("recovery draft exceeds the 128 MiB limit"));
    }
    let draft: Draft = serde_json::from_slice(&data).map_err(io::Error::other)?;
    let document = parse_svg(&draft.svg).map_err(io::Error::other)?;
    if !document
        .root()
        .and_then(|id| document.get_node(id))
        .is_some_and(|node| matches!(node.element, SvgElement::Svg(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "draft has no SVG root",
        ));
    }
    Ok(Some(Recovered {
        name: draft.name,
        document,
        slot,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_draft_is_hidden_and_a_recovered_draft_survives_until_clean_exit() {
        let folder = tempfile::tempdir().unwrap();
        let document = parse_svg("<svg><rect width='3' height='4'/></svg>").unwrap();
        let now = Instant::now();
        let mut session = Recovery::default();
        let due = session
            .schedule(now, true, Duration::from_secs(10), true, false)
            .unwrap();
        assert!(due > now);
        assert!(
            session
                .schedule(due, true, Duration::from_secs(10), true, true)
                .unwrap()
                > due
        );
        session
            .checkpoint(folder.path(), due, "Untitled.svg", &document)
            .unwrap();
        assert!(newest(folder.path()).unwrap().is_none());
        drop(session);

        let mut session = Recovery::default();
        let recovered = newest(folder.path()).unwrap().unwrap();
        let (name, recovered_document) = session.adopt(recovered);
        assert_eq!(name, "Untitled.svg");
        assert_eq!(serialize_svg(&recovered_document), serialize_svg(&document));
        assert!(newest(folder.path()).unwrap().is_none());
        drop(session);

        let mut session = Recovery::default();
        session.adopt(newest(folder.path()).unwrap().unwrap());
        session.reset().unwrap();
        assert!(newest(folder.path()).unwrap().is_none());
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 0);
    }
}
