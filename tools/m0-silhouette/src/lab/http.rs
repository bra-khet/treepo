//! Minimal blocking HTTP/1.1 over `std::net` — no external server crate
//! (`deny.toml` bans tiny_http/axum/etc. workspace-wide).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// One inbound request after headers (and optional body) are read.
#[derive(Debug)]
pub(super) struct Request {
    /// HTTP method, upper-cased.
    pub(super) method: String,
    /// Path only (query stripped).
    pub(super) path: String,
    /// Raw body bytes.
    pub(super) body: Vec<u8>,
}

/// Status line reason phrases we actually use.
fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

/// Bind and accept forever, calling `handler` per connection.
pub(super) fn serve<F>(addr: &str, mut handler: F) -> Result<(), String>
where
    F: FnMut(Request) -> Response,
{
    let listener = TcpListener::bind(addr).map_err(|e| format!("bind {addr}: {e}"))?;
    listener
        .set_nonblocking(false)
        .map_err(|e| format!("set blocking: {e}"))?;
    println!("m0-silhouette lab — http://{addr}/");
    println!("  Ctrl+C to stop\n");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
                match read_request(&mut stream) {
                    Ok(req) => {
                        let response = handler(req);
                        if let Err(e) = response.write_to(&mut stream) {
                            eprintln!("lab: write response: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("lab: bad request: {e}");
                        let _ =
                            Response::text(400, &format!("bad request: {e}")).write_to(&mut stream);
                    }
                }
            }
            Err(e) => eprintln!("lab: accept: {e}"),
        }
    }
    Ok(())
}

/// Outbound HTTP response.
#[derive(Debug)]
pub(super) struct Response {
    status: u16,
    content_type: String,
    body: Vec<u8>,
    extra_headers: Vec<(String, String)>,
}

impl Response {
    /// JSON body.
    pub(super) fn json(status: u16, body: &str) -> Self {
        Self {
            status,
            content_type: "application/json; charset=utf-8".to_owned(),
            body: body.as_bytes().to_vec(),
            extra_headers: Vec::new(),
        }
    }

    /// Plain text.
    pub(super) fn text(status: u16, body: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8".to_owned(),
            body: body.as_bytes().to_vec(),
            extra_headers: Vec::new(),
        }
    }

    /// Bytes with an explicit content type.
    pub(super) fn bytes(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type: content_type.to_owned(),
            body,
            extra_headers: Vec::new(),
        }
    }

    /// CORS-friendly JSON error.
    pub(super) fn error(status: u16, message: &str) -> Self {
        let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
        Self::json(status, &format!(r#"{{"error":"{escaped}"}}"#))
    }

    fn write_to(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\
             Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, OPTIONS\r\n\
             Access-Control-Allow-Headers: Content-Type\r\nCache-Control: no-store\r\n",
            self.status,
            reason(self.status),
            self.content_type,
            self.body.len()
        );
        for (k, v) in &self.extra_headers {
            head.push_str(k);
            head.push_str(": ");
            head.push_str(v);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes())?;
        stream.write_all(&self.body)?;
        stream.flush()?;
        Ok(())
    }
}

fn read_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut buf = Vec::with_capacity(8192);
    let mut chunk = [0u8; 4096];
    // Read until headers end or buffer grows large.
    loop {
        let n = stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 64 * 1024 {
            return Err("headers too large".to_owned());
        }
    }

    let header_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "incomplete headers".to_owned())?;
    let header_bytes = &buf[..header_end];
    let header_text = std::str::from_utf8(header_bytes).map_err(|_| "headers not utf-8")?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().ok_or_else(|| "empty request".to_owned())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_owned())?
        .to_ascii_uppercase();
    let target = parts
        .next()
        .ok_or_else(|| "missing target".to_owned())?
        .to_owned();
    let path = target
        .split_once('?')
        .map(|(p, _)| p.to_owned())
        .unwrap_or_else(|| target.clone());

    let mut content_length = 0usize;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }

    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream
            .read(&mut chunk)
            .map_err(|e| format!("read body: {e}"))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);

    Ok(Request { method, path, body })
}
