//! Load `qa/subjects.ron` into resolved filesystem paths.

use std::path::{Path, PathBuf};

/// One gallery subject after resolution.
#[derive(Debug, Clone)]
pub(super) struct Subject {
    /// Stable id used in PNG filenames and the UI.
    pub(super) id: String,
    /// Repository root to extract.
    pub(super) path: PathBuf,
    /// Optional coverage note from the subjects file.
    pub(super) covers: String,
}

/// Load subjects relative to the workspace root.
pub(super) fn load(workspace: &Path) -> Result<Vec<Subject>, String> {
    let file = workspace.join("qa").join("subjects.ron");
    let text =
        std::fs::read_to_string(&file).map_err(|e| format!("reading {}: {e}", file.display()))?;
    parse(&text, workspace)
}

fn parse(text: &str, workspace: &Path) -> Result<Vec<Subject>, String> {
    // Tiny RON scan: enough for the tracked subjects.ron shape, no serde dependency.
    // Entries look like:
    //   (id: "empty", corpus: "empty", covers: "…"),
    //   (id: "self", path: ".", covers: "…"),
    // Commented lines (`// …`) are skipped entirely before scanning for `(id:`.
    let mut subjects = Vec::new();
    let active: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut rest = active.as_str();
    while let Some(start) = rest.find("(id:") {
        rest = &rest[start..];
        let end = rest
            .find("),")
            .or_else(|| rest.find(")\n"))
            .or_else(|| rest.rfind(')'))
            .ok_or_else(|| "unterminated subject entry in subjects.ron".to_owned())?;
        let entry = &rest[..=end];
        rest = &rest[end + 1..];

        let id = field(entry, "id").ok_or_else(|| "subject missing id".to_owned())?;
        let covers = field(entry, "covers").unwrap_or_default();

        let path = if let Some(corpus) = field(entry, "corpus") {
            resolve_corpus(workspace, &corpus)?
        } else if let Some(rel) = field(entry, "path") {
            let p = PathBuf::from(&rel);
            if p.is_absolute() {
                p
            } else {
                workspace.join(p)
            }
        } else if let Some(pin) = field(entry, "pin") {
            resolve_pin(workspace, &pin)?
        } else {
            return Err(format!("subject `{id}` needs corpus, path, or pin"));
        };

        subjects.push(Subject { id, path, covers });
    }

    if subjects.is_empty() {
        return Err("no subjects found in qa/subjects.ron".to_owned());
    }
    Ok(subjects)
}

fn field(entry: &str, name: &str) -> Option<String> {
    let key = format!("{name}:");
    let at = entry.find(&key)?;
    let after = entry[at + key.len()..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let rest = &after[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn resolve_corpus(workspace: &Path, name: &str) -> Result<PathBuf, String> {
    let root = corpus::default_root();
    // Prefer workspace-relative default if corpus uses target/ under workspace.
    let _ = workspace;
    let fixtures = corpus::ensure(&root).map_err(|e| format!("building corpus: {e}"))?;
    fixtures
        .into_iter()
        .find(|f| f.name == name)
        .map(|f| f.path)
        .ok_or_else(|| format!("no corpus fixture named `{name}`"))
}

fn resolve_pin(_workspace: &Path, name: &str) -> Result<PathBuf, String> {
    let pins = corpus::Pins::built_in();
    let pin = pins
        .get(name)
        .ok_or_else(|| format!("no pin named `{name}`"))?;
    let root = corpus::pinned::default_root();
    match corpus::pinned::ensure(&root, pin) {
        corpus::Presence::Pinned => Ok(root.join(&pin.name)),
        _ => Err(format!(
            "pin `{name}` is not on disk; fetch it first with \
             `cargo run -p m0-silhouette -- --pin {name} --fetch`"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal() {
        let text = r#"
(
    version: 1,
    subjects: [
        (id: "empty", corpus: "empty", covers: "AC-SKEL-2"),
        // (id: "ripgrep", pin: "ripgrep", covers: "pin"),
        (id: "self", path: ".", covers: "real"),
    ],
)
"#;
        // Without a real corpus this only checks the path subject + comment skip when
        // resolve_corpus is not hit for empty — call field/parse structure via path only.
        let text_paths = r#"
(
    subjects: [
        (id: "self", path: ".", covers: "real"),
        // (id: "skip", path: "x", covers: "no"),
    ],
)
"#;
        let dir = std::env::temp_dir();
        let subjects = parse(text_paths, &dir).unwrap();
        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0].id, "self");
        let _ = text;
    }
}
