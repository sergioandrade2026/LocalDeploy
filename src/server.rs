use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};

pub fn run_from_args() -> Option<Result<(), String>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("--serve-directory") {
        return None;
    }
    if args.len() != 4 || args.get(2).map(String::as_str) != Some("--port") {
        return Some(Err("Uso interno inválido do servidor local.".into()));
    }
    let root = PathBuf::from(&args[1]);
    let port = args[3]
        .parse::<u16>()
        .map_err(|_| "Porta interna inválida.".to_string());
    Some(port.and_then(|port| serve(root, port)))
}

fn serve(root: PathBuf, port: u16) -> Result<(), String> {
    if !root.join("index.html").is_file() {
        return Err("O diretório estático não possui index.html.".into());
    }
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|error| format!("Não foi possível abrir a porta local: {error}"))?;
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let _ = respond(stream, &root);
            }
            Err(error) => return Err(format!("Erro no servidor local: {error}")),
        }
    }
    Ok(())
}

fn respond(mut stream: TcpStream, root: &Path) -> Result<(), String> {
    let mut request = [0_u8; 4096];
    let length = stream
        .read(&mut request)
        .map_err(|error| format!("Não foi possível ler a requisição local: {error}"))?;
    let request = String::from_utf8_lossy(&request[..length]);
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let target = target
        .split('?')
        .next()
        .unwrap_or("/")
        .trim_start_matches('/');
    let requested = safe_path(root, target).unwrap_or_else(|| root.join("index.html"));
    let path = if requested.is_file() {
        requested
    } else {
        root.join("index.html")
    };
    match fs::read(&path) {
        Ok(body) => write_response(&mut stream, "200 OK", mime_type(&path), &body),
        Err(_) => write_response(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "Arquivo não encontrado".as_bytes(),
        ),
    }
}

fn safe_path(root: &Path, target: &str) -> Option<PathBuf> {
    let target = Path::new(target);
    if target
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
    {
        Some(root.join(target))
    } else {
        None
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .and_then(|_| stream.write_all(body))
    .map_err(|error| format!("Não foi possível responder ao navegador: {error}"))
}

fn mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "wasm" => "application/wasm",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_allow_parent_directory_paths() {
        assert!(safe_path(Path::new("C:/preview"), "../secret").is_none());
        assert!(safe_path(Path::new("C:/preview"), "assets/app.js").is_some());
    }
}
