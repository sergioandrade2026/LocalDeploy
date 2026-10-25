// This binary intentionally keeps the console subsystem so stdio is reliable.
// The IDE launches it with windowsHide=true / CREATE_NO_WINDOW.
use localdeploy::integration::Engine;
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

fn main() {
    if let Some(result) = localdeploy::server::run_from_args() {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 || args[0] != "--data-dir" {
        eprintln!("Uso: localdeploy-engine --data-dir <pasta absoluta gerenciada pela IDE>");
        std::process::exit(2);
    }
    let engine = Engine::open(PathBuf::from(&args[1])).unwrap_or_else(|e| {
        eprintln!("{}", e.message);
        std::process::exit(1)
    });
    let output = Arc::new(Mutex::new(io::stdout()));
    let events = output.clone();
    engine.on_event(move |event| {
        let mut out = events.lock().expect("stdout mutex");
        let _ = writeln!(
            out,
            "{}",
            json!({"jsonrpc":"2.0","method":"localdeploy/event","params":event})
        );
        let _ = out.flush();
    });
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<Value>(&line);
        let mut shutdown = false;
        let response = match parsed {
            Err(_) => {
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"JSON inválido"}})
            }
            Ok(request) => {
                let id = request.get("id").cloned().unwrap_or(Value::Null);
                if request["jsonrpc"] != "2.0"
                    || !request["method"].is_string()
                    || request.get("id").is_none()
                {
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"Requisição JSON-RPC inválida"}})
                } else {
                    let method = request["method"].as_str().unwrap();
                    match engine
                        .request(method, request.get("params").cloned().unwrap_or(json!({})))
                    {
                        Ok(result) => {
                            shutdown = method == "shutdown";
                            json!({"jsonrpc":"2.0","id":id,"result":result})
                        }
                        Err(error) => json!({"jsonrpc":"2.0","id":id,"error":error}),
                    }
                }
            }
        };
        let mut out = output.lock().expect("stdout mutex");
        if writeln!(out, "{response}")
            .and_then(|_| out.flush())
            .is_err()
        {
            break;
        }
        if shutdown {
            break;
        }
    }
}
