//! `F-MAN-6` and `F-MAN-7` — reading and writing `manifest.bin`, versioned and atomically.
//!
//! Two files, per architecture D9/E2:
//!
//! * **`manifest.bin`** — an 11-byte header, then the manifest in postcard. Authoritative.
//! * **`manifest-meta.json`** — schema version, treepo version, and counts, for a person
//!   looking at the directory. Written, never read back.
//!
//! # The stored schema is not the model
//!
//! [`stored`] mirrors [`Manifest`](treepo_model::Manifest) as plain data — integers, strings,
//! vectors — and serde derives on that rather than on `treepo-model`. Three things follow, and
//! all three were the reason:
//!
//! * **`treepo-det` keeps its zero-dependency claim.** `Manifest` holds a `Seed`, an
//!   `OrderedMap`, and an `OrderedSet`, all defined in `treepo-det`. Deriving serde on
//!   `Manifest` means deriving it on those, and `cargo xtask dep-guard` asserts `treepo-det`
//!   has exactly one package in its graph — the crate every generated value flows through
//!   depends on nothing outside this workspace.
//! * **`schema_version` describes a type that exists to be the schema.** With serde on the
//!   model, adding a field silently changes the on-disk encoding and nothing prompts a version
//!   bump. Here, changing what is stored means editing a type whose only purpose is to be
//!   stored, in the file that holds `SCHEMA_VERSION`.
//! * **`N4`'s type discipline survives.** `AuthorShare` deliberately implements neither `Ord`
//!   nor `Serialize`; the mirror stores parts-per-million and the model stays as strict as it
//!   was.
//!
//! The cost is a conversion in each direction. It is paid down by destructuring: every
//! conversion below binds a struct's fields by name, so adding a field to `treepo-model` makes
//! this file stop compiling rather than quietly stop persisting. Types whose fields are
//! private are the exception, and are marked where they occur.
//!
//! # `F-MAN-6` — the version is in the header, not only in the sidecar
//!
//! > A schema mismatch triggers regeneration rather than a best-effort parse.
//!
//! That has to be decidable *before* the body is handed to a decoder, or "best-effort parse"
//! is exactly what happens. So the first eleven bytes are a magic and a fixed-width version,
//! readable on their own. The sidecar carries the version too, because `N2` promises a user
//! can see what treepo holds — but it is a copy for humans, and nothing reads it. A file a
//! person can edit must not be able to talk the loader into misparsing a manifest.
//!
//! # `F-MAN-7` — staged, then committed
//!
//! > Writes are atomic: temporary file, then rename. Thrive never observes a partially
//! > written manifest, and cancellation never leaves one.
//!
//! [`stage`] writes and flushes both files under temporary names; [`Staged::commit`] renames
//! them into place. Splitting it is what "cancellation never leaves one" needs: dropping a
//! [`Staged`] without committing removes the temporaries, so an abandoned write leaves the
//! previous store exactly as it was.
//!
//! The sidecar is renamed **first** and the manifest **last**, so the manifest's rename is the
//! single instant at which the store changes. A process killed between the two leaves a
//! sidecar describing a manifest that has not landed yet — cosmetic, because nothing reads it,
//! and the alternative would leave a manifest described by stale counts.

pub mod stored;

use crate::paths::RepositoryStore;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use treepo_model::Manifest;

/// Marks the file as treepo's and as a manifest.
///
/// The `0x1f` is deliberate: it is not printable, so a text editor and `file(1)` both report
/// binary rather than inviting a hand edit.
const MAGIC: [u8; 7] = *b"treepo\x1f";

/// `MAGIC`, then the schema version as little-endian `u32`.
const HEADER_LEN: usize = MAGIC.len() + 4;

/// Why a manifest could not be written.
#[derive(Debug)]
pub enum WriteError {
    /// The filesystem refused.
    Io {
        /// The file being written.
        path: PathBuf,
        /// What it said.
        source: std::io::Error,
    },
    /// The manifest could not be encoded.
    ///
    /// postcard fails only on an allocation failure or a `Serialize` impl that errors, and
    /// the mirror types have neither — so this is unreachable in practice and returned rather
    /// than unwrapped because "unreachable" and "cannot happen" are different claims.
    Encode(postcard::Error),
}

/// Why a stored manifest could not be read.
#[derive(Debug)]
pub enum ReadError {
    /// There is no manifest here. An ordinary first open, not a failure (`F-MAN-8`).
    Absent,
    /// The filesystem refused.
    Io {
        /// The file being read.
        path: PathBuf,
        /// What it said.
        source: std::io::Error,
    },
    /// The file is too short to hold a header, or does not start with one.
    NotAManifest {
        /// The file that was read.
        path: PathBuf,
    },
    /// Written by a different schema version (`F-MAN-6`).
    ///
    /// The caller regenerates. The body is deliberately not attempted.
    SchemaMismatch {
        /// The version in the file.
        found: u32,
        /// The version this build writes.
        expected: u32,
    },
    /// The header was right and the body was not — a truncated or corrupted file.
    ///
    /// PRD §6, "Store present but corrupt": regenerate rather than fail (`F-MAN-6`).
    Corrupt {
        /// The file that was read.
        path: PathBuf,
        /// What the decoder said.
        source: postcard::Error,
    },
    /// The body decoded but held a value the model rejects.
    ///
    /// A path that is not a valid [`RepoPath`](treepo_model::path::RepoPath), an object id of
    /// an impossible width, an enum discriminant this build does not know. Distinct from
    /// [`Corrupt`](Self::Corrupt) because it means the *encoding* was fine — which is the
    /// interesting case if it ever appears alongside a matching schema version.
    Invalid {
        /// What was wrong.
        detail: String,
    },
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "writing {}: {source}", path.display()),
            Self::Encode(source) => write!(f, "encoding the manifest: {source}"),
        }
    }
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent => write!(f, "no manifest stored yet"),
            Self::Io { path, source } => write!(f, "reading {}: {source}", path.display()),
            Self::NotAManifest { path } => {
                write!(f, "{} is not a treepo manifest", path.display())
            }
            Self::SchemaMismatch { found, expected } => write!(
                f,
                "stored manifest is schema {found}, this build writes {expected} — \
                 regenerating"
            ),
            Self::Corrupt { path, source } => {
                write!(f, "{} is corrupt: {source} — regenerating", path.display())
            }
            Self::Invalid { detail } => write!(f, "stored manifest is not usable: {detail}"),
        }
    }
}

impl std::error::Error for WriteError {}
impl std::error::Error for ReadError {}

impl ReadError {
    /// Whether the right response is to extract again (`F-MAN-6`, `F-MAN-8`).
    ///
    /// True for everything except an I/O failure, which will very likely repeat and which the
    /// user should be told about rather than have papered over with a long re-extraction.
    #[must_use]
    pub const fn is_regenerable(&self) -> bool {
        !matches!(self, Self::Io { .. })
    }
}

/// A manifest written to temporary files but not yet in place (`F-MAN-7`).
///
/// Dropping this without [`commit`](Self::commit) removes the temporaries, which is what makes
/// a cancelled write leave nothing behind.
#[derive(Debug)]
pub struct Staged {
    manifest_tmp: PathBuf,
    manifest_final: PathBuf,
    meta_tmp: PathBuf,
    meta_final: PathBuf,
    /// Size of the encoded manifest, for the caller's progress reporting.
    bytes: u64,
}

impl Staged {
    /// How many bytes the encoded manifest occupies.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Renames both files into place.
    ///
    /// # Errors
    ///
    /// [`WriteError::Io`] if a rename fails. The temporaries are left where they are: a
    /// failed rename means the filesystem is refusing, and deleting evidence at that point
    /// helps nobody.
    pub fn commit(self) -> Result<u64, WriteError> {
        // Sidecar first, manifest last — see the module docs.
        rename(&self.meta_tmp, &self.meta_final)?;
        rename(&self.manifest_tmp, &self.manifest_final)?;
        let bytes = self.bytes;
        std::mem::forget(self);
        Ok(bytes)
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        // Best effort by design: this runs on a cancelled write, and there is nobody to
        // report to. What matters is that the previous manifest is untouched, which it is
        // whether or not these succeed.
        let _ = std::fs::remove_file(&self.manifest_tmp);
        let _ = std::fs::remove_file(&self.meta_tmp);
    }
}

/// Writes a manifest into `store`, atomically (`F-MAN-7`).
///
/// Returns the size of `manifest.bin`.
///
/// # Errors
///
/// [`WriteError`] if the directory cannot be created, either file cannot be written, or the
/// manifest cannot be encoded.
pub fn write(store: &RepositoryStore, manifest: &Manifest) -> Result<u64, WriteError> {
    stage(store, manifest)?.commit()
}

/// Writes a manifest to temporary files without putting it in place (`F-MAN-7`).
///
/// # Errors
///
/// As [`write`].
pub fn stage(store: &RepositoryStore, manifest: &Manifest) -> Result<Staged, WriteError> {
    let dir = store.dir();
    std::fs::create_dir_all(dir).map_err(|source| WriteError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    let body = postcard::to_allocvec(&stored::StoredManifest::from(manifest))
        .map_err(WriteError::Encode)?;

    let mut bytes = Vec::with_capacity(HEADER_LEN + body.len());
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&manifest.schema_version.to_le_bytes());
    bytes.extend_from_slice(&body);

    let manifest_final = store.manifest_file();
    let meta_final = store.manifest_meta_file();
    let manifest_tmp = temporary(&manifest_final);
    let meta_tmp = temporary(&meta_final);

    // Constructed before either write, so a failure part-way through still runs `Drop` and
    // clears whichever temporary did get created.
    let staged = Staged {
        bytes: bytes.len() as u64,
        manifest_tmp,
        manifest_final,
        meta_tmp,
        meta_final,
    };

    write_durably(&staged.manifest_tmp, &bytes)?;
    write_durably(
        &staged.meta_tmp,
        meta_json(manifest, staged.bytes).as_bytes(),
    )?;
    Ok(staged)
}

/// Reads the manifest stored in `store` (`F-MAN-6`).
///
/// # Errors
///
/// [`ReadError::Absent`] when there is none — the ordinary first open. Every other variant
/// except [`ReadError::Io`] means "regenerate"; see [`ReadError::is_regenerable`].
pub fn read(store: &RepositoryStore) -> Result<Manifest, ReadError> {
    let path = store.manifest_file();
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReadError::Absent);
        }
        Err(source) => return Err(ReadError::Io { path, source }),
    };

    let schema_version =
        schema_version_of(&bytes).ok_or(ReadError::NotAManifest { path: path.clone() })?;
    if schema_version != treepo_model::manifest::SCHEMA_VERSION {
        return Err(ReadError::SchemaMismatch {
            found: schema_version,
            expected: treepo_model::manifest::SCHEMA_VERSION,
        });
    }

    let stored: stored::StoredManifest = postcard::from_bytes(&bytes[HEADER_LEN..])
        .map_err(|source| ReadError::Corrupt { path, source })?;
    stored
        .into_manifest(schema_version)
        .map_err(|detail| ReadError::Invalid { detail })
}

/// The schema version a stored manifest declares, without decoding it (`F-MAN-6`).
///
/// `None` if the bytes are not a manifest at all.
#[must_use]
pub fn schema_version_of(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < HEADER_LEN || bytes[..MAGIC.len()] != MAGIC {
        return None;
    }
    let mut version = [0u8; 4];
    version.copy_from_slice(&bytes[MAGIC.len()..HEADER_LEN]);
    Some(u32::from_le_bytes(version))
}

/// A temporary name beside `final_path`.
///
/// Process id and a counter rather than a random suffix: two treepo processes writing the same
/// store at once is unusual but not impossible, and this crate has no randomness that is not
/// also a determinism input. The suffix never survives the rename, so nothing downstream sees
/// it.
fn temporary(final_path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = final_path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}-{serial}.tmp", std::process::id()));
    final_path.with_file_name(name)
}

/// Writes `bytes` and does not return until they are on the device.
///
/// `sync_all` is what makes the rename meaningful. Without it the rename can be durable while
/// the contents are not, and a power failure leaves `manifest.bin` in place and empty — the
/// exact state `F-MAN-7` exists to prevent, reached by a shorter route than a partial write.
fn write_durably(path: &Path, bytes: &[u8]) -> Result<(), WriteError> {
    let io = |source| WriteError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file = File::create(path).map_err(io)?;
    file.write_all(bytes).map_err(io)?;
    file.sync_all().map_err(io)
}

fn rename(from: &Path, to: &Path) -> Result<(), WriteError> {
    std::fs::rename(from, to).map_err(|source| WriteError::Io {
        path: to.to_path_buf(),
        source,
    })
}

// ---------------------------------------------------------------------------------------
// The sidecar.
// ---------------------------------------------------------------------------------------

/// `manifest-meta.json` — what D9 says it carries, in a fixed field order.
///
/// Written by hand rather than through a JSON library. It is nine scalars that treepo never
/// reads back, so a serializer would be a dependency bought entirely for output this file can
/// produce in twenty lines. When a later phase needs to *read* `config.json` or
/// `settings.json` (`F-SET-*`), that is the moment to add one.
fn meta_json(manifest: &Manifest, manifest_bytes: u64) -> String {
    let commit = manifest
        .built_from_commit
        .map_or_else(|| "null".to_string(), |id| json_string(&id.to_string()));

    format!(
        "{{\n  \"schema_version\": {},\n  \"treepo_version\": {},\n  \
         \"built_from_commit\": {commit},\n  \"reference_time\": {},\n  \
         \"is_shallow\": {},\n  \"path_count\": {},\n  \"author_count\": {},\n  \
         \"language_count\": {},\n  \"manifest_bytes\": {manifest_bytes}\n}}\n",
        manifest.schema_version,
        json_string(&manifest.treepo_version),
        manifest.reference_time,
        manifest.is_shallow,
        manifest.paths().len(),
        manifest.authors.len(),
        manifest.languages.len(),
    )
}

/// A JSON string literal, escaped per RFC 8259.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use treepo_det::Seed;
    use treepo_det::{Fx, OrderedMap};
    use treepo_model::identity::{AuthorKey, CommitId};
    use treepo_model::manifest::{
        AuthorEntry, LanguageTable, Manifest, NodeKind, PathRecord, SCHEMA_VERSION,
    };
    use treepo_model::path::RepoPath;
    use treepo_model::primitives::size::{ContentCategory, SizeDistribution};
    use treepo_model::primitives::{
        BalanceScore, BranchingHistogram, ChurnWindows, ContentModulation, DepthProfile,
        FolderSignal, HierarchyPosition, LineCounts, OwnershipPrimitives,
    };

    fn scratch(name: &str) -> RepositoryStore {
        let dir = std::env::temp_dir().join("treepo-manifest-io").join(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("clearing scratch");
        }
        crate::StoreRoot::at(dir).repository(&treepo_model::identity::RepoIdentity::new(
            treepo_model::identity::IdentityTier::Remote,
            "example.com/x".to_string(),
            0,
        ))
    }

    fn path(s: &str) -> RepoPath {
        RepoPath::new(s.as_bytes()).expect("a valid path")
    }

    /// A manifest with **every** field set to something distinguishable.
    ///
    /// Defaults are what a round-trip test misses: a field that is dropped on the way out and
    /// defaulted on the way in compares equal if it was default to begin with. Nothing here
    /// is zero, empty, or `None` unless the `None` case is the one being exercised.
    fn populated() -> Manifest {
        let mut manifest = Manifest::new("9.9.9-test".to_string(), Seed::root(b"round-trip"));
        manifest.built_from_commit = Some(CommitId::sha1([0xa7; 20]));
        manifest.reference_time = 1_700_000_123;
        manifest.is_shallow = true;

        let ada = AuthorKey::from_email(b"ada@example.com");
        let bob = AuthorKey::from_email(b"bob@example.com");
        manifest.authors.insert(
            ada,
            AuthorEntry {
                recency: 1_699_000_000,
                commit_count: 41,
                is_self: true,
            },
        );
        manifest.authors.insert(
            bob,
            AuthorEntry {
                recency: 1_698_000_000,
                commit_count: 7,
                is_self: false,
            },
        );

        let mut languages = LanguageTable::new();
        let rust = languages.intern("Rust");
        let wgsl = languages.intern("WGSL");
        manifest.languages = languages;

        manifest.filters.use_gitignore = false;
        manifest
            .filters
            .extra_exclusions
            .insert("fixtures/**".into());
        manifest.filters.re_inclusions.insert("vendor/keep".into());

        let mut record = PathRecord::new(path("src/deep/main.rs"), NodeKind::File);
        record.structural.child_count = 3;
        record.structural.descendant_file_count = 9;
        record.structural.descendant_dir_count = 2;
        record.structural.max_subtree_depth = 5;
        record.structural.branching = BranchingHistogram::from_buckets([1, 2, 3, 4, 5, 6, 7, 8, 9]);
        record.structural.depth_profile = DepthProfile::from_levels(vec![4, 3, 2, 1]);
        record.structural.balance = BalanceScore {
            size: Fx::from_ratio(1, 3),
            depth: Fx::from_ratio(2, 7),
            kind: Some(Fx::from_ratio(5, 11)),
        };
        record.structural.hierarchy_skew = Fx::from_ratio(3, 8);

        record.size.bytes = 4_096;
        record.size.relative_bytes = Fx::from_ratio(1, 17);
        record.size.lines = LineCounts {
            total: 100,
            code: 70,
            comment: 20,
            blank: 10,
        };
        record.size.language_bytes = [(rust, 3_000u64), (wgsl, 1_096)].into_iter().collect();
        record.size.category_bytes = [
            (ContentCategory::Code, 4_000u64),
            (ContentCategory::Docs, 96),
        ]
        .into_iter()
        .collect();
        record.size.distribution = SizeDistribution {
            min: 1,
            median: 2,
            p90: 3,
            max: 4,
            mean: 5,
        };
        record.size.large_file_count = 6;

        record.temporal.first_commit_time = Some(1_600_000_000);
        record.temporal.last_commit_time = Some(1_690_000_000);
        record.temporal.commit_count = 33;
        record.temporal.churn = ChurnWindows {
            days_30: 11,
            days_90: 22,
            days_365: 33,
            lifetime: 44,
        };
        record.temporal.recency_heat = Fx::from_ratio(7, 9);
        record.temporal.burstiness = Fx::from_ratio(2, 5);
        record.temporal.stability = Some(Fx::from_ratio(4, 13));

        let counts: OrderedMap<AuthorKey, u64> = [(ada, 900u64), (bob, 100)].into_iter().collect();
        let recency: OrderedMap<AuthorKey, i64> = [(ada, 1_690_000_000i64), (bob, 1_680_000_000)]
            .into_iter()
            .collect();
        record.ownership = OwnershipPrimitives::from_line_counts(&counts, recency);

        record.derived.comment_density = Some(Fx::from_ratio(1, 5));
        record.derived.test_to_source = Some(Fx::from_ratio(2, 3));
        record.derived.todo_density = Some(Fx::from_ratio(1, 100));
        record.derived.doc_staleness_days = Some(97);
        record.derived.generated_debt = Some(Fx::from_ratio(1, 9));
        record.derived.large_file_debt = Some(Fx::from_ratio(3, 4));

        record.folder_signal = Some(FolderSignal {
            signal_name: "tests".into(),
            default_semantic_weight: Fx::from_ratio(19, 20),
            content_modulation: ContentModulation {
                language_concentration: Fx::from_ratio(1, 2),
                size_ratio: Fx::from_ratio(1, 3),
                binary_ratio: Fx::from_ratio(1, 4),
                test_like_ratio: Fx::from_ratio(1, 5),
                generated_ratio: Fx::from_ratio(1, 6),
            },
            effective_weight: Fx::from_ratio(9, 10),
            position_in_hierarchy: HierarchyPosition {
                depth: 2,
                ancestor_signals: Box::from(["vendor".into(), "docs".into()]),
            },
        });

        // A second record whose optional fields are all absent, so both branches of every
        // `Option` are encoded at least once.
        let bare = PathRecord::new(path("README.md"), NodeKind::File);
        manifest.set_paths(vec![record, bare]);
        manifest
    }

    #[test]
    fn a_fully_populated_manifest_round_trips_unchanged() {
        let store = scratch("round-trip");
        let original = populated();
        write(&store, &original).expect("the write");
        let read_back = read(&store).expect("the read");
        assert_eq!(original, read_back);
    }

    /// `AC-MAN-1`, at the encoding layer: the same manifest is always the same bytes.
    #[test]
    fn encoding_is_a_function_of_the_manifest_alone() {
        let first = scratch("identical-a");
        let second = scratch("identical-b");
        write(&first, &populated()).expect("first write");
        write(&second, &populated()).expect("second write");
        assert_eq!(
            std::fs::read(first.manifest_file()).expect("a"),
            std::fs::read(second.manifest_file()).expect("b"),
            "two stores, two paths, identical bytes"
        );
    }

    /// The encoding gate. Any change to `stored` or to postcard changes this digest, which
    /// forces the `SCHEMA_VERSION` bump `F-MAN-6` requires rather than leaving it to whoever
    /// made the change to remember.
    ///
    /// **If this test fails and the change was intentional, bump `SCHEMA_VERSION` and update
    /// the digest in the same commit.** A manifest written by the old build is not readable
    /// by the new one, and the version is what tells it to regenerate instead of misparse.
    #[test]
    fn the_encoding_has_a_golden_digest() {
        let body =
            postcard::to_allocvec(&stored::StoredManifest::from(&populated())).expect("encoding");
        let digest = treepo_det::hash::Sha256::digest(&body);
        let hex: String = digest
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            hex, "30151920b619082a8c252a9568c7e365106ec2b1643bd660fd47aebbca89cdbf",
            "schema {SCHEMA_VERSION} encoding changed — see this test's documentation"
        );
    }

    /// `AC-MAN-3` — killing the process mid-write leaves the previous store intact and valid.
    ///
    /// `mem::forget` is the point: a killed process runs no destructor, so the temporary is
    /// left behind exactly as it would be. Dropping normally is the *cancellation* case, and
    /// is checked separately below.
    #[test]
    fn a_write_killed_before_its_rename_leaves_the_previous_manifest() {
        let store = scratch("killed");
        let mut first = populated();
        first.treepo_version = "first".to_string();
        write(&store, &first).expect("the first write");

        let mut second = populated();
        second.treepo_version = "second".to_string();
        second.reference_time = 1;
        let staged = stage(&store, &second).expect("the staging");
        let leftover = staged.manifest_tmp.clone();
        std::mem::forget(staged);

        assert!(leftover.exists(), "the interruption was real");
        let recovered = read(&store).expect("the previous manifest is still readable");
        assert_eq!(recovered, first, "and it is the previous one, unchanged");
    }

    /// The other half of `F-MAN-7`: "cancellation never leaves one".
    #[test]
    fn an_abandoned_write_leaves_nothing_behind() {
        let store = scratch("cancelled");
        write(&store, &populated()).expect("the first write");

        let staged = stage(&store, &populated()).expect("the staging");
        let (manifest_tmp, meta_tmp) = (staged.manifest_tmp.clone(), staged.meta_tmp.clone());
        drop(staged);

        assert!(!manifest_tmp.exists());
        assert!(!meta_tmp.exists());
        assert!(read(&store).is_ok(), "and the store is still whole");
    }

    /// `F-MAN-6` — a mismatched schema regenerates rather than best-effort parses.
    #[test]
    fn a_future_schema_is_refused_without_being_parsed() {
        let store = scratch("future-schema");
        write(&store, &populated()).expect("the write");

        let path = store.manifest_file();
        let mut bytes = std::fs::read(&path).expect("reading it back");
        bytes[MAGIC.len()..HEADER_LEN].copy_from_slice(&99u32.to_le_bytes());
        std::fs::write(&path, &bytes).expect("rewriting the header");

        match read(&store) {
            Err(error @ ReadError::SchemaMismatch { found: 99, .. }) => {
                assert!(error.is_regenerable());
                assert!(error.to_string().contains("regenerating"));
            }
            other => panic!("expected a schema mismatch, got {other:?}"),
        }
        assert_eq!(schema_version_of(&bytes), Some(99));
    }

    /// PRD §6, "Store present but corrupt": regenerate rather than fail.
    #[test]
    fn a_truncated_or_foreign_file_is_regenerable_rather_than_fatal() {
        let store = scratch("corrupt");
        write(&store, &populated()).expect("the write");
        let path = store.manifest_file();
        let whole = std::fs::read(&path).expect("reading it back");

        std::fs::write(&path, &whole[..whole.len() / 2]).expect("truncating");
        let truncated = read(&store).expect_err("a truncated body");
        assert!(matches!(truncated, ReadError::Corrupt { .. }));
        assert!(truncated.is_regenerable());

        std::fs::write(&path, b"just some other file").expect("replacing");
        let foreign = read(&store).expect_err("not a manifest");
        assert!(matches!(foreign, ReadError::NotAManifest { .. }));
        assert!(foreign.is_regenerable());

        std::fs::remove_file(&path).expect("removing");
        let absent = read(&store).expect_err("nothing stored");
        assert!(matches!(absent, ReadError::Absent));
        assert!(absent.is_regenerable(), "a first open extracts");
    }

    /// `N2` — a person can see what treepo holds.
    #[test]
    fn the_sidecar_is_readable_and_says_what_d9_requires() {
        let store = scratch("sidecar");
        let manifest = populated();
        let bytes = write(&store, &manifest).expect("the write");

        let text = std::fs::read_to_string(store.manifest_meta_file()).expect("the sidecar");
        for expected in [
            &format!("\"schema_version\": {SCHEMA_VERSION}"),
            "\"treepo_version\": \"9.9.9-test\"",
            "\"path_count\": 2",
            "\"author_count\": 2",
            "\"language_count\": 2",
            "\"is_shallow\": true",
            &format!("\"manifest_bytes\": {bytes}"),
            "\"built_from_commit\": \"a7a7a7",
        ] {
            assert!(
                text.contains(expected),
                "sidecar is missing {expected}:\n{text}"
            );
        }
        assert_eq!(
            bytes,
            std::fs::metadata(store.manifest_file())
                .expect("stat")
                .len(),
            "the reported size is the file's size"
        );
    }

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(json_string("plain"), "\"plain\"");
        assert_eq!(json_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(json_string("line\nbreak"), "\"line\\nbreak\"");
        assert_eq!(json_string("\u{1}"), "\"\\u0001\"");
    }

    /// Two writers must not truncate each other's temporary file.
    #[test]
    fn temporary_names_do_not_collide() {
        let target = Path::new("/store/manifest.bin");
        let first = temporary(target);
        let second = temporary(target);
        assert_ne!(first, second);
        assert_eq!(first.parent(), target.parent(), "staged beside its target");
        for name in [&first, &second] {
            let name = name.file_name().and_then(|n| n.to_str()).expect("a name");
            assert!(name.starts_with("manifest.bin."));
            assert!(name.ends_with(".tmp"));
        }
    }
}
