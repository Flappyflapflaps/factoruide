//! `fg-web-server`: the web app (a folder of static files) on a server of its own.
//!
//!   fg-web-server [--dir web/dist] [--listen 127.0.0.1:8080] [--upstream https://contrib.example]
//!
//! - every answer is cross-origin isolated (the backend's threads share memory, which browsers
//!   allow only then);
//! - `POST /v1/contributions` and `POST /v1/issues` go on to `--upstream` (the contribution
//!   server), without anything about the player (no address, no cookie, a fixed user agent), and
//!   its answer comes back as it is. Without `--upstream`, they are refused.
//!
//! Meant to run behind an HTTPS proxy (browsers isolate pages only on HTTPS or localhost).

use anyhow::{Context, Result};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

/// Largest body passed on (a contribution document is at most a few megabytes).
const MAX_BODY: u64 = 16 << 20;

struct Config {
    dir: PathBuf,
    upstream: Option<String>,
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (mut dir, mut listen, mut upstream) = (PathBuf::from("web/dist"), "127.0.0.1:8080".to_owned(), None);
    while let Some(arg) = args.next() {
        let mut value = || args.next().with_context(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--dir" => dir = PathBuf::from(value()?),
            "--listen" => listen = value()?,
            "--upstream" => upstream = Some(value()?.trim_end_matches('/').to_owned()),
            "-h" | "--help" => {
                println!("fg-web-server [--dir web/dist] [--listen 127.0.0.1:8080] [--upstream URL]");
                return Ok(());
            }
            other => anyhow::bail!("unknown argument {other} (see --help)"),
        }
    }
    anyhow::ensure!(
        dir.join("index.html").is_file(),
        "{}: no index.html (build it with scripts/build-web.sh)",
        dir.display()
    );
    let server = Server::http(&listen).map_err(|e| anyhow::anyhow!("{listen}: {e}"))?;
    println!("Factoruide on http://{listen}/ ({})", dir.display());
    let config = std::sync::Arc::new(Config { dir, upstream });
    for request in server.incoming_requests() {
        let config = std::sync::Arc::clone(&config);
        std::thread::spawn(move || {
            if let Err(e) = handle(&config, request) {
                eprintln!("{e:#}");
            }
        });
    }
    Ok(())
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("a valid header")
}

/// The headers of every answer.
fn isolated<R: Read>(mut response: Response<R>) -> Response<R> {
    for (name, value) in [
        ("Cross-Origin-Opener-Policy", "same-origin"),
        ("Cross-Origin-Embedder-Policy", "require-corp"),
        ("Cross-Origin-Resource-Policy", "same-origin"),
        ("X-Content-Type-Options", "nosniff"),
    ] {
        response.add_header(header(name, value));
    }
    response
}

fn handle(config: &Config, mut request: Request) -> Result<()> {
    let path = request.url().split(['?', '#']).next().unwrap_or("/").to_owned();
    if let Some(endpoint) = path
        .strip_prefix("/v1/")
        .filter(|p| matches!(*p, "contributions" | "issues"))
    {
        if *request.method() != Method::Post {
            return Ok(request.respond(isolated(Response::empty(405)))?);
        }
        let Some(upstream) = &config.upstream else {
            return Ok(request.respond(isolated(json(503, "no contribution server configured")))?);
        };
        let mut body = Vec::new();
        request.as_reader().take(MAX_BODY + 1).read_to_end(&mut body)?;
        if body.len() as u64 > MAX_BODY {
            return Ok(request.respond(isolated(json(413, "too large")))?);
        }
        let answer = forward(&format!("{upstream}/v1/{endpoint}"), &body);
        return Ok(request.respond(isolated(answer))?);
    }
    if !matches!(request.method(), Method::Get | Method::Head) {
        return Ok(request.respond(isolated(Response::empty(405)))?);
    }
    match file(&config.dir, &path) {
        Some(file) => {
            let data = std::fs::read(&file)?;
            let mut response = Response::from_data(data).with_header(header("Content-Type", content_type(&file)));
            // Hashed names never change; the rest is checked again.
            let immutable = path.starts_with("/assets/");
            response.add_header(header(
                "Cache-Control",
                if immutable {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-cache"
                },
            ));
            Ok(request.respond(isolated(response))?)
        }
        None => Ok(request.respond(isolated(Response::from_string("not found").with_status_code(404)))?),
    }
}

/// The file of `dir` a URL path names (`/` is index.html); none outside `dir`.
fn file(dir: &Path, path: &str) -> Option<PathBuf> {
    let path = percent_decode(path)?;
    let mut out = dir.to_owned();
    for part in Path::new(path.trim_start_matches('/')).components() {
        match part {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.is_dir() {
        out.push("index.html");
    }
    out.is_file().then_some(out)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let text = String::from_utf8(out).ok()?;
    (!text.contains('\0') && !text.contains('\\')).then_some(text)
}

fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "png" => "image/png",
        "md" => "text/markdown; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn json(status: u16, error: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(format!("{{\"error\":{error:?}}}"))
        .with_status_code(StatusCode(status))
        .with_header(header("Content-Type", "application/json"))
}

/// Post `body` to the contribution server; its status, `Retry-After` and body come back.
fn forward(url: &str, body: &[u8]) -> Response<std::io::Cursor<Vec<u8>>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .proxy(None)
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent("Factoruide")
        .http_status_as_error(false)
        .build()
        .into();
    match agent.post(url).header("Content-Type", "application/json").send(body) {
        Ok(mut r) => {
            let status = r.status().as_u16();
            let retry = r
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let data = r
                .body_mut()
                .with_config()
                .limit(1 << 20)
                .read_to_vec()
                .unwrap_or_default();
            let mut response = Response::from_data(data)
                .with_status_code(StatusCode(status))
                .with_header(header("Content-Type", "application/json"));
            if let Some(retry) = retry {
                response.add_header(header("Retry-After", &retry));
            }
            response
        }
        Err(e) => json(502, &format!("contribution server: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_files_inside_the_folder() {
        let dir = std::env::temp_dir().join(format!("fg-web-server-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), "x").unwrap();
        std::fs::write(dir.join("assets/a b.js"), "x").unwrap();
        assert_eq!(file(&dir, "/"), Some(dir.join("index.html")));
        assert_eq!(file(&dir, "/assets/a%20b.js"), Some(dir.join("assets/a b.js")));
        assert_eq!(file(&dir, "/../etc/passwd"), None);
        assert_eq!(file(&dir, "/%2e%2e/etc/passwd"), None);
        assert_eq!(file(&dir, "/assets/..%5c..%5cx"), None);
        assert_eq!(file(&dir, "/missing.js"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
