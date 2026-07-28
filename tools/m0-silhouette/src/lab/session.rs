//! Session workspace under `qa/sessions/`: experiment tables, renders, findings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use treepo_gen::Table;
use treepo_model::Manifest;

use crate::lab::json::{Request as JsonReq, Value};
use crate::lab::subjects::Subject;
use crate::lab::table_edit;
use crate::pipeline::{self, short_digest};

// Session folder stamps are human labels only — not generative inputs. clippy.toml bans
// wall-clock types from the product path; the lab is a tools/ harness and names directories.
#[allow(clippy::disallowed_types, clippy::disallowed_methods)]
fn wall_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Lab process state: one active session plus cached manifests.
#[derive(Debug)]
pub(super) struct LabState {
    /// Workspace root (contains `qa/`, `assets/`).
    pub(super) workspace: PathBuf,
    /// Resolved gallery subjects.
    pub(super) subjects: Vec<Subject>,
    /// Product table path used as default seed.
    pub(super) table_source: PathBuf,
    /// Canvas edge in pixels.
    pub(super) size: u32,
    /// Active session, if any.
    pub(super) session: Option<Session>,
    /// Extracted manifests keyed by subject id (warm after first render).
    manifests: BTreeMap<String, Manifest>,
}

/// One tuning session on disk.
#[derive(Debug)]
pub(super) struct Session {
    /// Directory name under `qa/sessions/`.
    pub(super) name: String,
    /// Absolute session directory.
    pub(super) dir: PathBuf,
    /// Working parameter table.
    pub(super) table: Table,
    /// Table at session start.
    pub(super) baseline: Table,
    /// Next render index (1-based on disk as 0001).
    pub(super) next_render: u32,
    /// Locked family for §6 discipline.
    pub(super) family: Option<String>,
    /// Focused parameter path.
    pub(super) parameter: Option<String>,
    /// Optional free-form notes.
    pub(super) notes: String,
    /// Subject ids included in this session.
    pub(super) subject_ids: Vec<String>,
}

/// Options for starting the lab.
#[derive(Debug, Clone)]
pub(super) struct LabOptions {
    /// Workspace root.
    pub(super) workspace: PathBuf,
    /// Initial table file (usually product `lsystem.ron`).
    pub(super) table_source: PathBuf,
    /// Canvas size.
    pub(super) size: u32,
    /// Optional session label.
    pub(super) label: String,
}

impl LabState {
    /// Load subjects and seed table; create the first session.
    pub(super) fn open(options: &LabOptions) -> Result<Self, String> {
        let subjects = crate::lab::subjects::load(&options.workspace)?;
        let table_text = std::fs::read_to_string(&options.table_source)
            .map_err(|e| format!("reading table {}: {e}", options.table_source.display()))?;
        let table = Table::from_ron(&table_text)
            .map_err(|e| format!("parameter table is not usable — {e}"))?;

        let mut state = Self {
            workspace: options.workspace.clone(),
            subjects,
            table_source: options.table_source.clone(),
            size: options.size,
            session: None,
            manifests: BTreeMap::new(),
        };
        state.create_session(&options.label, table)?;
        Ok(state)
    }

    /// Create a new session directory and make it active.
    pub(super) fn create_session(&mut self, label: &str, table: Table) -> Result<&Session, String> {
        let stamp = timestamp_stamp();
        let safe_label = sanitize_label(label);
        let name = format!("{stamp}_{safe_label}");
        let dir = self.workspace.join("qa").join("sessions").join(&name);
        std::fs::create_dir_all(dir.join("renders"))
            .map_err(|e| format!("creating session: {e}"))?;
        std::fs::create_dir_all(dir.join("findings"))
            .map_err(|e| format!("creating findings dir: {e}"))?;

        let ron = table_edit::to_ron(&table);
        std::fs::write(dir.join("experiment.ron"), &ron)
            .map_err(|e| format!("writing experiment.ron: {e}"))?;
        std::fs::write(dir.join("baseline.ron"), &ron)
            .map_err(|e| format!("writing baseline.ron: {e}"))?;

        let subject_ids: Vec<String> = self.subjects.iter().map(|s| s.id.clone()).collect();
        let meta = format!(
            "{{\n  \"started_at\": \"{}\",\n  \"label\": {},\n  \"table_source\": {},\n  \
             \"subjects\": {},\n  \"notes\": \"\"\n}}\n",
            iso_now(),
            json_string(label),
            json_string(&self.table_source.display().to_string()),
            json_string_array(&subject_ids),
        );
        std::fs::write(dir.join("meta.json"), meta)
            .map_err(|e| format!("writing meta.json: {e}"))?;

        // Optional pointer for humans / agents.
        let current = self.workspace.join("qa").join("current");
        let _ = std::fs::write(&current, format!("{name}\n"));

        self.session = Some(Session {
            name,
            dir,
            baseline: table.clone(),
            table,
            next_render: 1,
            family: None,
            parameter: None,
            notes: String::new(),
            subject_ids,
        });
        Ok(self.session.as_ref().expect("just set"))
    }

    fn session_mut(&mut self) -> Result<&mut Session, String> {
        self.session
            .as_mut()
            .ok_or_else(|| "no active session".to_owned())
    }

    fn session_ref(&self) -> Result<&Session, String> {
        self.session
            .as_ref()
            .ok_or_else(|| "no active session".to_owned())
    }

    /// Persist the working experiment table to disk.
    fn write_experiment(&self) -> Result<(), String> {
        let session = self.session_ref()?;
        let ron = table_edit::to_ron(&session.table);
        std::fs::write(session.dir.join("experiment.ron"), ron)
            .map_err(|e| format!("writing experiment.ron: {e}"))
    }

    /// Ensure manifests for all session subjects are extracted.
    fn warm_manifests(&mut self) -> Result<(), String> {
        let ids: Vec<String> = self.session_ref()?.subject_ids.clone();
        for id in ids {
            if self.manifests.contains_key(&id) {
                continue;
            }
            let subject = self
                .subjects
                .iter()
                .find(|s| s.id == id)
                .ok_or_else(|| format!("unknown subject `{id}`"))?;
            eprintln!(
                "lab: extracting subject `{id}` from {} …",
                subject.path.display()
            );
            let manifest = pipeline::manifest_for(&subject.path)?;
            self.manifests.insert(id, manifest);
        }
        Ok(())
    }

    /// JSON snapshot of the active session for the UI.
    pub(super) fn session_json(&self) -> Result<String, String> {
        let session = self.session_ref()?;
        let mut root = Value::object();
        root.insert("session", session.name.as_str());
        root.insert("dir", session.dir.display().to_string());
        root.insert("table_source", self.table_source.display().to_string());
        root.insert("size", self.size);
        root.insert("next_render", session.next_render);
        root.insert(
            "family",
            session
                .family
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        );
        root.insert(
            "parameter",
            session
                .parameter
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        );
        root.insert("notes", session.notes.as_str());

        let subjects: Vec<Value> = self
            .subjects
            .iter()
            .filter(|s| session.subject_ids.iter().any(|id| id == &s.id))
            .map(|s| {
                let mut o = Value::object();
                o.insert("id", s.id.as_str());
                o.insert("path", s.path.display().to_string());
                o.insert("covers", s.covers.as_str());
                o
            })
            .collect();
        root.insert("subjects", subjects);

        let families = families_json();
        root.insert("families", families);

        let fields = fields_json(&session.table, session.family.as_deref());
        root.insert("fields", fields);

        let renders = list_renders(&session.dir)?;
        root.insert("renders", renders);

        let findings = list_findings(&session.dir)?;
        root.insert("findings", findings);

        let diffs = table_edit::diff_summary(&session.baseline, &session.table);
        let mut diff_obj = Value::object();
        for (path, from, to) in diffs {
            let mut pair = Value::object();
            pair.insert("from", from);
            pair.insert("to", to);
            diff_obj.insert(path, pair);
        }
        root.insert("table_diff", diff_obj);

        Ok(root.encode())
    }

    /// Lock a family (or clear with empty).
    pub(super) fn set_family(&mut self, family: Option<String>) -> Result<(), String> {
        let session = self.session_mut()?;
        if let Some(ref f) = family {
            let known = table_edit::catalog().iter().any(|c| c.family == f);
            if !known {
                return Err(format!("unknown family `{f}`"));
            }
        }
        session.family = family;
        if let Some(ref param) = session.parameter.clone() {
            let still = table_edit::catalog().iter().any(|c| {
                c.path == param.as_str() && session.family.as_deref().is_none_or(|f| c.family == f)
            });
            if !still {
                session.parameter = None;
            }
        }
        Ok(())
    }

    /// Focus one parameter path.
    pub(super) fn set_parameter(&mut self, path: Option<String>) -> Result<(), String> {
        let session = self.session_mut()?;
        if let Some(ref p) = path {
            let meta = table_edit::catalog()
                .iter()
                .find(|c| c.path == p.as_str())
                .ok_or_else(|| format!("unknown parameter `{p}`"))?;
            if let Some(ref family) = session.family {
                if meta.family != family.as_str() {
                    return Err(format!(
                        "`{p}` is family {}, session is locked to {family}",
                        meta.family
                    ));
                }
            } else {
                session.family = Some(meta.family.to_owned());
            }
        }
        session.parameter = path;
        Ok(())
    }

    /// Set a field value on the experiment table.
    pub(super) fn set_field(&mut self, path: &str, value: i32) -> Result<String, String> {
        {
            let session = self.session_mut()?;
            if let Some(ref family) = session.family {
                let meta = table_edit::catalog()
                    .iter()
                    .find(|c| c.path == path)
                    .ok_or_else(|| format!("unknown parameter `{path}`"))?;
                if meta.family != family.as_str() {
                    return Err(format!(
                        "family lock: session is `{family}`, field is `{}`",
                        meta.family
                    ));
                }
            }
            table_edit::set_value_validated(&mut session.table, path, value)?;
            session.parameter = Some(path.to_owned());
            if session.family.is_none()
                && let Some(meta) = table_edit::catalog().iter().find(|c| c.path == path)
            {
                session.family = Some(meta.family.to_owned());
            }
        }
        self.write_experiment()?;
        self.session_json()
    }

    /// Render the multi-subject strip into the next non-overwriting directory.
    pub(super) fn render(&mut self, notes: &str) -> Result<String, String> {
        self.warm_manifests()?;
        let size = self.size;
        let table_source = "experiment.ron".to_owned();

        let (index, render_dir, family, parameter, table_ron, subject_plan) = {
            let session = self.session_ref()?;
            let index = session.next_render;
            let render_dir = session.dir.join("renders").join(format!("{index:04}"));
            let family = session.family.clone().unwrap_or_else(|| "unset".to_owned());
            let parameter = session
                .parameter
                .clone()
                .unwrap_or_else(|| "unset".to_owned());
            let table_ron = table_edit::to_ron(&session.table);
            let subject_plan: Vec<(String, PathBuf)> = session
                .subject_ids
                .iter()
                .filter_map(|id| {
                    self.subjects
                        .iter()
                        .find(|s| &s.id == id)
                        .map(|s| (s.id.clone(), s.path.clone()))
                })
                .collect();
            (
                index,
                render_dir,
                family,
                parameter,
                table_ron,
                subject_plan,
            )
        };

        std::fs::create_dir_all(&render_dir).map_err(|e| format!("creating render dir: {e}"))?;
        std::fs::write(render_dir.join("experiment.ron"), &table_ron)
            .map_err(|e| format!("snapshot experiment.ron: {e}"))?;

        let table = {
            let session = self.session_ref()?;
            session.table.clone()
        };

        let mut subjects_meta = Value::object();
        for (id, path) in &subject_plan {
            let manifest = self
                .manifests
                .get(id)
                .ok_or_else(|| format!("manifest missing for `{id}`"))?;
            let rendered =
                pipeline::render_manifest(id, path, manifest, &table, &table_source, size);
            let png_name = format!("{id}.png");
            std::fs::write(render_dir.join(&png_name), &rendered.png)
                .map_err(|e| format!("writing {png_name}: {e}"))?;
            std::fs::write(render_dir.join(format!("{id}.txt")), &rendered.sidecar)
                .map_err(|e| format!("writing sidecar: {e}"))?;

            let mut one = Value::object();
            one.insert("png", png_name.as_str());
            one.insert("skeleton_digest", rendered.report.digest.to_string());
            one.insert("short_digest", short_digest(rendered.report.digest));
            one.insert("paths", rendered.report.paths);
            one.insert("nodes", rendered.report.nodes);
            one.insert("segments", rendered.report.segments);
            one.insert("depth", i64::from(rendered.report.depth));
            subjects_meta.insert(id.as_str(), one);
        }

        let mut meta = Value::object();
        meta.insert("index", index);
        meta.insert("family", family.as_str());
        meta.insert("parameter", parameter.as_str());
        meta.insert("notes", notes);
        meta.insert("subjects", subjects_meta);
        std::fs::write(render_dir.join("meta.json"), meta.encode())
            .map_err(|e| format!("writing render meta: {e}"))?;

        {
            let session = self.session_mut()?;
            session.next_render = index + 1;
            if !notes.is_empty() {
                session.notes = notes.to_owned();
            }
        }

        eprintln!(
            "lab: render {index:04} family={family} param={parameter} → {}",
            render_dir.display()
        );
        self.session_json()
    }

    /// Export a finding JSON for agent handoff.
    pub(super) fn export(&mut self, body: &JsonReq) -> Result<String, String> {
        let session_name = self.session_ref()?.name.clone();
        let family = body
            .str("family")
            .map(str::to_owned)
            .or_else(|| self.session_ref().ok().and_then(|s| s.family.clone()))
            .ok_or_else(|| "family required".to_owned())?;
        let parameter = body
            .str("parameter")
            .map(str::to_owned)
            .or_else(|| self.session_ref().ok().and_then(|s| s.parameter.clone()))
            .ok_or_else(|| "parameter required".to_owned())?;
        let verdict = body.str("verdict").unwrap_or("prefer");
        match verdict {
            "prefer" | "reject" | "blocked" | "needs_code" => {}
            other => return Err(format!("invalid verdict `{other}`")),
        }
        let notes = body.str("notes").unwrap_or("").to_owned();
        let chosen_render = body.require_str("chosen_render")?.to_owned();
        // Normalise to renders/NNNN
        let chosen = if chosen_render.starts_with("renders/") {
            chosen_render
        } else {
            format!("renders/{chosen_render}")
        };

        let session = self.session_ref()?;
        let render_path = session.dir.join(&chosen);
        if !render_path.is_dir() {
            return Err(format!(
                "chosen_render `{chosen}` is not a render directory"
            ));
        }

        let mut chosen_subjects = Value::object();
        for id in &session.subject_ids {
            let png = format!("{id}.png");
            if render_path.join(&png).is_file() {
                chosen_subjects.insert(id.as_str(), format!("{chosen}/{png}"));
            }
        }

        let mut table_diff = Value::object();
        for (path, from, to) in table_edit::diff_summary(&session.baseline, &session.table) {
            let mut pair = Value::object();
            pair.insert("from", from);
            pair.insert("to", to);
            table_diff.insert(path, pair);
        }

        let mut finding = Value::object();
        finding.insert("session", session_name.as_str());
        finding.insert("family", family.as_str());
        finding.insert("parameter", parameter.as_str());
        finding.insert("verdict", verdict);
        finding.insert("chosen_render", chosen.as_str());
        finding.insert("chosen_subjects", chosen_subjects);
        finding.insert("table_diff_summary", table_diff);
        finding.insert("notes", notes.as_str());
        if let Some(rej) = body.str("rejected_renders") {
            // comma-separated optional
            let arr: Vec<Value> = rej
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(Value::from)
                .collect();
            finding.insert("rejected_renders", arr);
        }

        let file_stem = format!(
            "{}_{}",
            sanitize_label(&family),
            sanitize_label(&parameter.replace('.', "-"))
        );
        let out_path = session
            .dir
            .join("findings")
            .join(format!("{file_stem}.json"));
        let encoded = finding.encode();
        // Pretty-ish: just write compact JSON; schema allows it.
        let pretty = format!("{encoded}\n");
        std::fs::write(&out_path, &pretty).map_err(|e| format!("writing finding: {e}"))?;

        let mut resp = Value::object();
        resp.insert("path", out_path.display().to_string());
        resp.insert("relative", format!("findings/{file_stem}.json"));
        resp.insert("finding", finding);
        Ok(resp.encode())
    }

    /// Serve a file from the session tree (PNG, etc.).
    pub(super) fn session_file(&self, rel: &str) -> Result<(String, Vec<u8>), String> {
        let session = self.session_ref()?;
        // Reject path escape.
        if rel.contains("..") || Path::new(rel).is_absolute() {
            return Err("invalid path".to_owned());
        }
        let path = session.dir.join(rel);
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let ctype = match path.extension().and_then(|e| e.to_str()) {
            Some("png") => "image/png",
            Some("json") => "application/json; charset=utf-8",
            Some("ron") => "text/plain; charset=utf-8",
            Some("txt") => "text/plain; charset=utf-8",
            _ => "application/octet-stream",
        };
        Ok((ctype.to_owned(), bytes))
    }
}

fn families_json() -> Value {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for field in table_edit::catalog() {
        if !seen.iter().any(|s: &String| s == field.family) {
            seen.push(field.family.to_owned());
            out.push(Value::from(field.family));
        }
    }
    Value::Array(out)
}

fn fields_json(table: &Table, family_lock: Option<&str>) -> Value {
    let mut arr = Vec::new();
    for field in table_edit::catalog() {
        if let Some(lock) = family_lock
            && field.family != lock
        {
            continue;
        }
        let value = table_edit::get_value(table, field.path).unwrap_or(0);
        let mut o = Value::object();
        o.insert("path", field.path);
        o.insert("family", field.family);
        o.insert("unit", field.unit);
        o.insert("soft_min", field.soft_min);
        o.insert("soft_max", field.soft_max);
        o.insert("step", field.step);
        o.insert("value", value);
        arr.push(o);
    }
    Value::Array(arr)
}

fn list_renders(session_dir: &Path) -> Result<Value, String> {
    let renders_dir = session_dir.join("renders");
    let mut entries = Vec::new();
    if renders_dir.is_dir() {
        let mut names: Vec<_> = std::fs::read_dir(&renders_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for name in names {
            let meta_path = renders_dir.join(&name).join("meta.json");
            let mut o = Value::object();
            o.insert("id", name.as_str());
            o.insert("path", format!("renders/{name}"));
            if let Ok(text) = std::fs::read_to_string(meta_path) {
                o.insert("meta_raw", text);
            }
            entries.push(o);
        }
    }
    Ok(Value::Array(entries))
}

fn list_findings(session_dir: &Path) -> Result<Value, String> {
    let dir = session_dir.join("findings");
    let mut entries = Vec::new();
    if dir.is_dir() {
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json"))
            .collect();
        names.sort();
        for name in names {
            entries.push(Value::from(format!("findings/{name}")));
        }
    }
    Ok(Value::Array(entries))
}

fn sanitize_label(label: &str) -> String {
    let s: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "session".to_owned()
    } else {
        s
    }
}

fn timestamp_stamp() -> String {
    let secs = wall_secs();
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let hh = day_secs / 3600;
    let mm = (day_secs % 3600) / 60;
    let ss = day_secs % 60;
    format!("{y:04}{m:02}{d:02}_{hh:02}{mm:02}{ss:02}")
}

fn iso_now() -> String {
    let secs = wall_secs();
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let hh = day_secs / 3600;
    let mm = (day_secs % 3600) / 60;
    let ss = day_secs % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Howard Hinnant civil_from_days (UTC).
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn json_string(s: &str) -> String {
    Value::from(s).encode()
}

fn json_string_array(items: &[String]) -> String {
    let arr: Vec<Value> = items.iter().map(|s| Value::from(s.as_str())).collect();
    Value::Array(arr).encode()
}

/// Dispatch an HTTP API call against lab state.
pub(super) fn handle_api(
    state: &mut LabState,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<String, String> {
    let body_text = std::str::from_utf8(body).unwrap_or("");
    match (method, path) {
        ("GET", "/api/session") => state.session_json(),
        ("POST", "/api/session") => {
            let req = if body_text.trim().is_empty() {
                JsonReq::default()
            } else {
                JsonReq::parse(body_text)?
            };
            let label = req.str("label").unwrap_or("session");
            let table = Table::built_in();
            state.create_session(label, table)?;
            state.session_json()
        }
        ("PUT", "/api/family") => {
            let req = JsonReq::parse(body_text)?;
            let family = req
                .str("family")
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            state.set_family(family)?;
            state.session_json()
        }
        ("PUT", "/api/parameter") => {
            let req = JsonReq::parse(body_text)?;
            let parameter = req
                .str("parameter")
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            state.set_parameter(parameter)?;
            state.session_json()
        }
        ("PUT", "/api/field") => {
            let req = JsonReq::parse(body_text)?;
            let path = req.require_str("path")?;
            let value = req.require_int("value")?;
            let value = i32::try_from(value).map_err(|_| "value out of i32 range".to_owned())?;
            state.set_field(path, value)
        }
        ("POST", "/api/render") => {
            let req = if body_text.trim().is_empty() {
                JsonReq::default()
            } else {
                JsonReq::parse(body_text)?
            };
            let notes = req.str("notes").unwrap_or("");
            state.render(notes)
        }
        ("POST", "/api/export") => {
            let req = JsonReq::parse(body_text)?;
            state.export(&req)
        }
        _ => Err(format!("no API route {method} {path}")),
    }
}
