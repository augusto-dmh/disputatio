//! Filesystem adapter for the Obsidian vault (specs/queue-slice/design.md:
//! `vault_fs.rs` walks courses + videos + scope.md). All IO lives here; the
//! domain receives pure data only.
//!
//! Vault contract (specs/queue-slice/requirements.md):
//! - Class tracks: `courses/<track>/sessions/<session>/` — one session
//!   directory is one class is one chunk. Course order comes from the
//!   session directory's leading number (`05-foo` → ord 5), falling back to
//!   sorted enumeration.
//! - Video track: `courses/videos/**` — one video file is one chunk.
//! - Seeding: a session with a `scope.md` whose task checkboxes are all
//!   checked (and at least one exists) was already studied → chunk seeded
//!   `done`; anything else stays `queued`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::chunk::{Chunk, ChunkStatus};
use crate::domain::source::{Source, SourceKind};
use crate::domain::vault::{ScannedSource, VaultError, VaultReader, VaultScan};

/// The video track directory: `courses/videos` (one video = one chunk).
const VIDEO_TRACK: &str = "videos";

/// Video file extensions indexed as chunks. Anything else under the video
/// folder (subtitles, thumbnails, notes) is not a chunk.
const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "mkv", "webm", "mov", "avi", "m4v"];

pub struct FsVaultReader;

impl VaultReader for FsVaultReader {
    fn scan(&self, vault_root: &Path) -> Result<VaultScan, VaultError> {
        if !vault_root.is_dir() {
            return Err(VaultError::NotFound(vault_root.display().to_string()));
        }
        let courses_dir = vault_root.join("courses");
        if !courses_dir.is_dir() {
            return Ok(VaultScan::default());
        }

        let mut track_dirs: Vec<PathBuf> = fs::read_dir(&courses_dir)
            .map_err(VaultError::Io)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        track_dirs.sort();

        let mut sources = Vec::new();
        let mut ord = 0;
        for track_dir in &track_dirs {
            let track = path_name(track_dir);
            if track == VIDEO_TRACK {
                continue; // the video track is appended last (design.md rotation)
            }
            let chunks = walk_class_track(vault_root, track_dir)?;
            if chunks.is_empty() {
                continue;
            }
            sources.push(ScannedSource {
                source: Source {
                    kind: SourceKind::Course,
                    track: track.clone(),
                    title: titleize(&track),
                    vault_path: format!("courses/{track}"),
                    ord,
                },
                chunks,
            });
            ord += 1;
        }

        let videos_dir = courses_dir.join(VIDEO_TRACK);
        if videos_dir.is_dir() {
            let chunks = walk_video_track(vault_root, &videos_dir)?;
            if !chunks.is_empty() {
                sources.push(ScannedSource {
                    source: Source {
                        kind: SourceKind::Video,
                        track: VIDEO_TRACK.to_string(),
                        title: "Videos".to_string(),
                        vault_path: format!("courses/{VIDEO_TRACK}"),
                        ord,
                    },
                    chunks,
                });
            }
        }

        Ok(VaultScan { sources })
    }
}

/// One class = one chunk per session directory under `courses/<track>/sessions/`.
fn walk_class_track(vault_root: &Path, track_dir: &Path) -> Result<Vec<Chunk>, VaultError> {
    let sessions_dir = track_dir.join("sessions");
    if !sessions_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut session_dirs: Vec<PathBuf> = fs::read_dir(&sessions_dir)
        .map_err(VaultError::Io)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    session_dirs.sort();

    let mut chunks = Vec::new();
    for (i, session_dir) in session_dirs.iter().enumerate() {
        let dir_name = path_name(session_dir);
        let scope_path = session_dir.join("scope.md");
        // No scope.md — class not studied yet; title falls back to the dir name.
        let scope = fs::read_to_string(&scope_path).ok();
        let status = scope
            .as_deref()
            .map(seeded_status)
            .unwrap_or(ChunkStatus::Queued);
        let title = scope
            .as_deref()
            .and_then(heading_title)
            .unwrap_or_else(|| title_from_dir(&dir_name));
        chunks.push(Chunk {
            ord: leading_number(&dir_name).unwrap_or(i as i32 + 1),
            title,
            vault_path: relative(vault_root, session_dir),
            est_minutes: None, // no duration source in the vault contract yet
            status,
        });
    }
    Ok(chunks)
}

/// One video = one chunk, recursively under `courses/videos/` (requirements:
/// "courses/videos (one video = one chunk)").
fn walk_video_track(vault_root: &Path, videos_dir: &Path) -> Result<Vec<Chunk>, VaultError> {
    let mut video_files: Vec<PathBuf> = Vec::new();
    collect_video_files(videos_dir, &mut video_files)?;
    video_files.sort();

    Ok(video_files
        .into_iter()
        .enumerate()
        .map(|(i, path)| Chunk {
            ord: i as i32 + 1,
            title: titleize(&path.file_stem().unwrap_or_default().to_string_lossy()),
            vault_path: relative(vault_root, &path),
            est_minutes: None,
            status: ChunkStatus::Queued, // videos have no scope.md seeding
        })
        .collect())
}

fn collect_video_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), VaultError> {
    for entry in fs::read_dir(dir).map_err(VaultError::Io)? {
        let path = entry.map_err(VaultError::Io)?.path();
        if path.is_dir() {
            collect_video_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| {
            VIDEO_EXTENSIONS.contains(&ext.to_string_lossy().to_lowercase().as_str())
        }) {
            out.push(path);
        }
    }
    Ok(())
}

/// The seeding rule: a session is already studied when its `scope.md` has a
/// task list and every item is checked (design.md: "parse scope.md to mark
/// already-studied classes done").
pub(crate) fn seeded_status(scope_md: &str) -> ChunkStatus {
    let (mut checked, mut unchecked) = (0usize, 0usize);
    for line in scope_md.lines() {
        let line = line.trim_start();
        let Some(rest) = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| line.strip_prefix("+ "))
        else {
            continue;
        };
        let rest = rest.trim_start();
        if rest.starts_with("[ ]") {
            unchecked += 1;
        } else if rest.starts_with("[x]") || rest.starts_with("[X]") {
            checked += 1;
        }
    }
    if checked > 0 && unchecked == 0 {
        ChunkStatus::Done
    } else {
        ChunkStatus::Queued
    }
}

/// First markdown `#` heading, trimmed — the session's own title when the
/// scope file declares one.
fn heading_title(scope_md: &str) -> Option<String> {
    scope_md
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("# ").map(str::trim))
        .filter(|t| !t.is_empty())
        .map(ToString::to_string)
}

/// `05-what-is-an-enterprise` → 5 (course order anchored on session numbers).
fn leading_number(name: &str) -> Option<i32> {
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// `fundamentos-enterprise` → `Fundamentos Enterprise`.
fn titleize(raw: &str) -> String {
    raw.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(first) => first.to_uppercase().collect::<String>() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Session-dir fallback title: drop the course-order number, keep the rest —
/// `05-what-is-an-enterprise` → `What Is An Enterprise`.
fn title_from_dir(dir_name: &str) -> String {
    let stripped = dir_name
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start_matches(['-', '_', ' ']);
    if stripped.is_empty() {
        titleize(dir_name)
    } else {
        titleize(stripped)
    }
}

/// Vault-relative, forward-slash path — the stable key shape persisted on
/// sources/chunks (ADR 0003).
fn relative(vault_root: &Path, path: &Path) -> String {
    path.strip_prefix(vault_root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn path_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Unique tempdir without extra dev-dependencies.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "disputatio-vaultfs-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&base).unwrap();
            TempDir(base)
        }

        fn write(&self, rel: &str, contents: &str) -> PathBuf {
            let path = self.0.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scan_at(root: &Path) -> VaultScan {
        FsVaultReader.scan(root).unwrap()
    }

    #[test]
    fn missing_root_is_an_error() {
        let err = FsVaultReader
            .scan(Path::new("/nonexistent/disputatio/vault"))
            .unwrap_err();
        assert!(matches!(err, VaultError::NotFound(_)));
    }

    #[test]
    fn empty_vault_scans_to_nothing() {
        let tmp = TempDir::new("empty");
        let scan = scan_at(&tmp.0);
        assert!(scan.sources.is_empty());
    }

    #[test]
    fn class_track_one_session_dir_is_one_chunk_with_course_order_and_seeding() {
        let tmp = TempDir::new("classes");
        tmp.write(
            "courses/fundamentos-enterprise/sessions/01-o-que-e-empresa/scope.md",
            "# O que é empresa\n\n- [x] Ler as notas de aula\n- [x] Narrar o resumo\n",
        );
        tmp.write(
            "courses/fundamentos-enterprise/sessions/02-modelos-de-negocio/scope.md",
            "# Modelos de negócio\n\n- [x] Ler as notas\n- [ ] Narrar o resumo\n",
        );
        tmp.write(
            "courses/fundamentos-enterprise/sessions/03-proposito/scope.md",
            "# Propósito\n\n- [ ] Ler as notas\n",
        );
        // Session without scope.md: not studied yet, title from the dir name.
        tmp.write(
            "courses/fundamentos-enterprise/sessions/04-jurassic/notes.md",
            "notes",
        );

        let scan = scan_at(&tmp.0);
        assert_eq!(scan.sources.len(), 1);
        let scanned = &scan.sources[0];
        assert_eq!(scanned.source.kind, SourceKind::Course);
        assert_eq!(scanned.source.track, "fundamentos-enterprise");
        assert_eq!(scanned.source.title, "Fundamentos Enterprise");
        assert_eq!(scanned.source.vault_path, "courses/fundamentos-enterprise");
        assert_eq!(scanned.source.ord, 0);

        let chunks = &scanned.chunks;
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].ord, 1);
        assert_eq!(chunks[0].title, "O que é empresa");
        assert_eq!(
            chunks[0].status,
            ChunkStatus::Done,
            "fully checked scope → done"
        );
        assert_eq!(
            chunks[1].status,
            ChunkStatus::Queued,
            "partially checked → queued"
        );
        assert_eq!(chunks[2].status, ChunkStatus::Queued);
        assert_eq!(
            chunks[3].title, "Jurassic",
            "title from dir name when no scope.md"
        );
        assert_eq!(
            chunks[3].status,
            ChunkStatus::Queued,
            "no scope.md → queued"
        );
        assert_eq!(
            chunks[2].vault_path,
            "courses/fundamentos-enterprise/sessions/03-proposito"
        );
        assert_eq!(scan.done_count(), 1);
    }

    #[test]
    fn sessions_without_leading_numbers_get_stable_enumerated_order() {
        let tmp = TempDir::new("nonum");
        tmp.write(
            "courses/system-design/sessions/intro/scope.md",
            "- [ ] ler\n",
        );
        tmp.write(
            "courses/system-design/sessions/escala/scope.md",
            "- [ ] ler\n",
        );
        tmp.write(
            "courses/system-design/sessions/balanco/scope.md",
            "- [ ] ler\n",
        );

        let scan = scan_at(&tmp.0);
        let chunks = &scan.sources[0].chunks;
        let ords: Vec<i32> = chunks.iter().map(|c| c.ord).collect();
        assert_eq!(ords, vec![1, 2, 3], "sorted dirs, 1-based enumeration");
        assert_eq!(
            chunks[0].title, "Balanco",
            "alphabetical order without leading numbers"
        );
    }

    #[test]
    fn video_track_one_video_is_one_chunk_recursive_and_sorted() {
        let tmp = TempDir::new("videos");
        tmp.write("courses/videos/modulo-a/b.mp4", "v1");
        tmp.write("courses/videos/a.mkv", "v2");
        tmp.write("courses/videos/modulo-b/sub/c.MP4", "v3");
        tmp.write("courses/videos/ignore-me.txt", "not a video");
        tmp.write("courses/videos/cover.jpg", "not a video");

        let scan = scan_at(&tmp.0);
        assert_eq!(scan.sources.len(), 1);
        let scanned = &scan.sources[0];
        assert_eq!(scanned.source.kind, SourceKind::Video);
        assert_eq!(scanned.source.track, "videos");
        assert_eq!(scanned.source.ord, 0); // no class tracks precede the videos

        let chunks = &scanned.chunks;
        assert_eq!(chunks.len(), 3, "one video = one chunk; non-videos ignored");
        let paths: Vec<&str> = chunks.iter().map(|c| c.vault_path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "courses/videos/a.mkv",
                "courses/videos/modulo-a/b.mp4",
                "courses/videos/modulo-b/sub/c.MP4"
            ]
        );
        assert!(chunks.iter().all(|c| c.status == ChunkStatus::Queued));
        assert_eq!(chunks[0].title, "A");
        assert_eq!(chunks[2].ord, 3);
    }

    #[test]
    fn class_and_video_tracks_coexist_videos_last() {
        let tmp = TempDir::new("mixed");
        tmp.write(
            "courses/system-design/sessions/01-intro/scope.md",
            "- [x] ok\n",
        );
        tmp.write("courses/videos/v.mp4", "v");
        tmp.write(
            "courses/fundamentos-enterprise/sessions/01-a/scope.md",
            "- [ ] a\n",
        );

        let scan = scan_at(&tmp.0);
        let tracks: Vec<&str> = scan
            .sources
            .iter()
            .map(|s| s.source.track.as_str())
            .collect();
        assert_eq!(
            tracks,
            vec!["fundamentos-enterprise", "system-design", "videos"]
        );
        let ords: Vec<i32> = scan.sources.iter().map(|s| s.source.ord).collect();
        assert_eq!(ords, vec![0, 1, 2]);
        assert_eq!(scan.chunk_count(), 3);
    }

    #[test]
    fn non_session_files_under_a_track_are_not_chunks() {
        let tmp = TempDir::new("assets");
        tmp.write(
            "courses/fundamentos-enterprise/README.md",
            "top-level, not a session",
        );
        tmp.write("courses/fundamentos-enterprise/assets/img.png", "binary");
        let scan = scan_at(&tmp.0);
        assert!(
            scan.sources.is_empty(),
            "track with zero sessions is skipped"
        );
    }
}
