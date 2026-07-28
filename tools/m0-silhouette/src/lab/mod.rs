//! Local HTML silhouette lab: family-locked sliders, non-overwriting renders, finding export.
//!
//! ```text
//! cargo run -p m0-silhouette -- lab
//! cargo run -p m0-silhouette -- lab --port 7420 --label crown
//! ```
//!
//! Serves static assets from `tools/m0-silhouette/lab/` and writes all working data under
//! `qa/sessions/` (gitignored). Render happens in-process via the same pipeline as the CLI.

mod http;
mod json;
mod session;
mod subjects;
mod table_edit;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use session::{LabOptions, LabState, handle_api};

/// Entry point for `m0-silhouette lab …`.
pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let workspace = workspace_root();
    let port = value(args, "--port")?.unwrap_or_else(|| "7420".to_owned());
    let host = value(args, "--host")?.unwrap_or_else(|| "127.0.0.1".to_owned());
    let label = value(args, "--label")?.unwrap_or_else(|| "lab".to_owned());
    let size: u32 = match value(args, "--size")? {
        Some(text) => text
            .parse()
            .map_err(|_| format!("--size wants a number of pixels, got `{text}`"))?,
        None => 768,
    };
    if !(64..=4096).contains(&size) {
        return Err(format!("--size {size} is outside 64..=4096"));
    }

    let table_source = match value(args, "--table")? {
        Some(path) => PathBuf::from(path),
        None => workspace.join("assets").join("params").join("lsystem.ron"),
    };

    let bind = format!("{host}:{port}");
    let options = LabOptions {
        workspace: workspace.clone(),
        table_source,
        size,
        label,
    };

    let state = LabState::open(&options)?;
    let session_name = state
        .session
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_default();
    println!("m0-silhouette lab");
    println!("  session  qa/sessions/{session_name}");
    println!("  table    {}", options.table_source.display());
    println!("  canvas   {size}×{size}");
    println!("  subjects {}", state.subjects.len());

    let state = Arc::new(Mutex::new(state));
    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lab");

    http::serve(&bind, move |req| {
        if req.method == "OPTIONS" {
            return http::Response::text(204, "");
        }

        // Static UI
        if req.method == "GET" && (req.path == "/" || req.path == "/index.html") {
            return serve_static(&static_dir, "index.html");
        }
        if req.method == "GET" && req.path.starts_with("/static/") {
            let name = &req.path["/static/".len()..];
            return serve_static(&static_dir, name);
        }

        // Session files (PNGs, meta, findings)
        if req.method == "GET" && req.path.starts_with("/files/") {
            let rel = &req.path["/files/".len()..];
            let mut guard = match state.lock() {
                Ok(g) => g,
                Err(_) => return http::Response::error(500, "state lock poisoned"),
            };
            // touch session access
            let _ = &mut *guard;
            return match guard.session_file(rel) {
                Ok((ctype, bytes)) => http::Response::bytes(200, &ctype, bytes),
                Err(e) => http::Response::error(404, &e),
            };
        }

        // API
        if req.path.starts_with("/api/") {
            let mut guard = match state.lock() {
                Ok(g) => g,
                Err(_) => return http::Response::error(500, "state lock poisoned"),
            };
            return match handle_api(&mut guard, &req.method, &req.path, &req.body) {
                Ok(body) => http::Response::json(200, &body),
                Err(e) => {
                    let status = if e.starts_with("no API route") {
                        404
                    } else {
                        400
                    };
                    http::Response::error(status, &e)
                }
            };
        }

        http::Response::error(404, &format!("not found: {}", req.path))
    })
}

fn serve_static(dir: &Path, name: &str) -> http::Response {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return http::Response::error(400, "bad path");
    }
    let path = dir.join(name);
    match std::fs::read(&path) {
        Ok(bytes) => {
            let ctype = match path.extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html; charset=utf-8",
                Some("js") => "text/javascript; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };
            http::Response::bytes(200, ctype, bytes)
        }
        Err(_) => http::Response::error(404, &format!("missing static asset `{name}`")),
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate is two levels below the workspace root")
        .to_path_buf()
}

fn value(args: &[String], flag: &str) -> Result<Option<String>, String> {
    match args.iter().position(|arg| arg == flag) {
        None => Ok(None),
        Some(at) => args
            .get(at + 1)
            .filter(|next| !next.starts_with("--"))
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("{flag} wants a value")),
    }
}

/// Usage lines for the lab subcommand.
pub(crate) fn print_usage() {
    println!(
        "m0-silhouette lab — local HTML harness for one-family silhouette tuning.

    cargo run -p m0-silhouette -- lab
    cargo run -p m0-silhouette -- lab --port 7420 --label crown
    cargo run -p m0-silhouette -- lab --table path/to/experiment.ron --size 768

  --host <addr>    bind address                 [127.0.0.1]
  --port <n>       bind port                    [7420]
  --label <name>   session label                [lab]
  --table <file>   seed parameter table         [assets/params/lsystem.ron]
  --size <px>      canvas edge                  [768]

  Sessions land in qa/sessions/<stamp>_<label>/ (gitignored).
  Static UI: tools/m0-silhouette/lab/
  Product table is never written by the lab — export findings, promote by hand."
    );
}
