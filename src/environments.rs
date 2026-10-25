use crate::process::hidden_command;
use std::{
    fs::{self, File},
    path::Path,
    process::{Child, Stdio},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    Node,
    RustWeb,
    DioxusWeb,
    DioxusDesktop,
}

pub fn detect(root: &Path) -> Result<Backend, String> {
    if root.join("Dioxus.toml").exists() {
        let raw = fs::read_to_string(root.join("Dioxus.toml")).map_err(|e| e.to_string())?;
        let config: toml::Value =
            toml::from_str(&raw).map_err(|e| format!("Dioxus.toml inválido: {e}"))?;
        return match config.get("application").and_then(|v| v.get("default_platform")).and_then(toml::Value::as_str) {
            Some("desktop") => Ok(Backend::DioxusDesktop),
            Some("web") => Ok(Backend::DioxusWeb),
            other => Err(format!("Defina application.default_platform como web ou desktop no Dioxus.toml (atual: {other:?}).")),
        };
    }
    if root.join("package.json").exists() {
        return Ok(Backend::Node);
    }
    if root.join("Cargo.toml").exists() {
        return Ok(Backend::RustWeb);
    }
    Err("Não há package.json, Cargo.toml ou Dioxus.toml na raiz. Monorepos precisam indicar a pasta da aplicação.".into())
}

pub fn prerequisites(backend: &Backend) -> Result<(), String> {
    let program = if *backend == Backend::DioxusDesktop {
        "cargo"
    } else {
        "docker"
    };
    let args: &[&str] = if program == "docker" {
        &["info", "--format", "{{.OSType}}"]
    } else {
        &["--version"]
    };
    let output = hidden_command(program).args(args).output().map_err(|e| {
        format!("{program} indisponível: {e}. Use Preparar ambiente em Configurações.")
    })?;
    if !output.status.success() {
        return Err(format!("{program} não está pronto: {}. Inicie Docker Desktop com backend WSL2 e contêineres Linux.", String::from_utf8_lossy(&output.stderr)));
    }
    if program == "docker" && String::from_utf8_lossy(&output.stdout).trim() != "linux" {
        return Err(
            "Docker está no modo Windows. Selecione contêineres Linux no Docker Desktop.".into(),
        );
    }
    Ok(())
}

fn image(root: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    Ok(format!(
        "localdeploy:{}",
        &format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes()))[..24]
    ))
}

pub fn build(root: &Path, tests: bool, log: &mut File) -> Result<(), String> {
    let backend = detect(root)?;
    prerequisites(&backend)?;
    let remote_supabase = fs::read_to_string(root.join(".env"))
        .unwrap_or_default()
        .lines()
        .any(|line| {
            line.split_once('=').is_some_and(|(key, value)| {
                key.ends_with("SUPABASE_URL") && !value.trim().is_empty()
            })
        });
    if root.join("supabase/config.toml").exists()
        && !remote_supabase
        && !root.join("compose.yaml").exists()
        && !root.join("docker-compose.yml").exists()
    {
        return Err("Supabase local detectado. Bloqueio: este projeto não fornece Compose com serviços e portas isolados por preview. Forneça compose.yaml para banco/auth/storage ou configure um Supabase remoto em .env; migrações não serão aplicadas à produção automaticamente.".into());
    }
    for example in [".env.example", ".env.local.example"] {
        if let Ok(content) = fs::read_to_string(root.join(example)) {
            let actual = fs::read_to_string(root.join(".env")).unwrap_or_default();
            let keys = actual
                .lines()
                .filter_map(|line| line.trim().split_once('=').map(|(k, _)| k.to_owned()))
                .collect::<std::collections::HashSet<_>>();
            let missing = content
                .lines()
                .filter_map(|line| line.trim().split_once('='))
                .filter(|(k, v)| {
                    !k.starts_with('#')
                        && !k.is_empty()
                        && v.trim().is_empty()
                        && !keys.contains(*k)
                })
                .map(|(k, _)| k)
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(format!("Variáveis obrigatórias sem valor: {}. Preencha .env no workspace; valores não são exibidos nos logs.", missing.join(", ")));
            }
        }
    }
    start_services(root, log)?;
    if backend == Backend::DioxusDesktop {
        if !hidden_command("dx")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            run("cargo", &["install", "dioxus-cli", "--locked"], root, log)?;
        }
        run("dx", &["build", "--platform", "desktop"], root, log)?;
        if tests {
            run("cargo", &["test"], root, log)?;
        }
        return Ok(());
    }
    let info = crate::detector::analyze(root);
    let recipe = match backend {
        Backend::Node => {
            let manifest: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(root.join("package.json")).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let manager = info.package_manager.as_deref().unwrap_or("npm");
            if !manager
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '-' | '+'))
            {
                return Err("Gerenciador inválido".into());
            }
            let binary = manager.split('@').next().unwrap_or("npm");
            let install = match binary {
                "npm" if root.join("package-lock.json").exists() => "npm ci",
                "npm" => "npm install",
                "pnpm" => "pnpm install --frozen-lockfile",
                "yarn" => "yarn install",
                "bun" => "bun install --frozen-lockfile",
                _ => return Err("Gerenciador não suportado".into()),
            };
            let provision = if binary == "npm" {
                String::new()
            } else {
                format!("RUN npm install -g {manager}\n")
            };
            let test = if tests && manifest["scripts"]["test"].is_string() {
                format!("RUN {binary} run test\n")
            } else {
                String::new()
            };
            let start = if info.technologies.iter().any(|t| t == "Vite") {
                format!("{binary} run dev -- --host 0.0.0.0 --port 3000")
            } else if info.technologies.iter().any(|t| t == "Next.js") {
                format!("{binary} run start -- --hostname 0.0.0.0 --port 3000")
            } else {
                format!("{binary} run start")
            };
            let version = info.node_version.as_deref().unwrap_or("22");
            let major = version
                .trim_start_matches(|c: char| !c.is_ascii_digit())
                .split('.')
                .next()
                .unwrap_or("22");
            let major: u32 = major
                .parse()
                .map_err(|_| "Versão Node precisa declarar uma versão principal numérica")?;
            format!("FROM node:{major}-bookworm\nWORKDIR /app\nCOPY . .\n{provision}RUN {install}\nRUN {binary} run build\n{test}ENV PORT=3000 HOST=0.0.0.0\nCMD {}\n", serde_json::to_string(&vec!["sh", "-c", &start]).map_err(|e| e.to_string())?)
        }
        Backend::RustWeb | Backend::DioxusWeb => {
            let test = if tests { "RUN cargo test\n" } else { "" };
            if backend == Backend::DioxusWeb {
                format!("FROM rust:slim-bookworm\nRUN apt-get update && apt-get install -y pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*\nWORKDIR /app\nCOPY . .\nRUN rustup target add wasm32-unknown-unknown && cargo install dioxus-cli --locked\nRUN dx build --platform web\n{test}CMD [\"dx\",\"serve\",\"--platform\",\"web\",\"--addr\",\"0.0.0.0\",\"--port\",\"3000\"]\n")
            } else {
                format!("FROM rust:slim-bookworm\nRUN apt-get update && apt-get install -y pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*\nWORKDIR /app\nCOPY . .\nRUN cargo build --release\n{test}ENV PORT=3000 HOST=0.0.0.0\nCMD [\"cargo\",\"run\",\"--release\"]\n")
            }
        }
        Backend::DioxusDesktop => unreachable!(),
    };
    let recipe_path = root.join(".localdeploy.Dockerfile");
    let recipe = if root.join(".env").is_file() {
        recipe.replace(
            "RUN ",
            "RUN --mount=type=secret,id=localdeploy_env,target=/app/.env ",
        )
    } else {
        recipe
    };
    fs::write(&recipe_path, recipe).map_err(|e| e.to_string())?;
    let existing = fs::read_to_string(root.join(".dockerignore")).unwrap_or_default();
    fs::write(root.join(".localdeploy.Dockerfile.dockerignore"), format!("{existing}\n.git\nnode_modules\ntarget\n.env\n.env.*\n*.log\n.localdeploy-cancel\n.localdeploy-progress\n")).map_err(|e| e.to_string())?;
    let image = image(root)?;
    let mut args = vec![
        "build",
        "--tag",
        &image,
        "--file",
        ".localdeploy.Dockerfile",
    ];
    if root.join(".env").is_file() {
        args.extend(["--secret", "id=localdeploy_env,src=.env"]);
    }
    args.push(".");
    run("docker", &args, root, log)
}

pub fn start(root: &Path, port: u16, log: &File) -> Result<Child, String> {
    let backend = detect(root)?;
    let mut command = if backend == Backend::DioxusDesktop {
        let mut command = hidden_command("dx");
        command.args(["serve", "--platform", "desktop"]);
        command
    } else {
        let mut command = hidden_command("docker");
        // Runtime variables are passed from the reserved workspace, never baked
        // into image layers or interpolated into a shell command.
        command.arg("run");
        if root.join(".env").is_file() {
            command.arg("--env-file").arg(root.join(".env"));
        }
        command.args([
            "--rm",
            "--name",
            &format!("{}-preview", image(root)?.replace(':', "-")),
            "--publish",
            &format!("127.0.0.1:{port}:3000"),
            &image(root)?,
        ]);
        command
    };
    command
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .spawn()
        .map_err(|e| e.to_string())
}

fn run(program: &str, args: &[&str], root: &Path, log: &mut File) -> Result<(), String> {
    crate::executor::run_command(program, args, root, log)
}

fn start_services(root: &Path, log: &mut File) -> Result<(), String> {
    let Some(source) = ["compose.yaml", "compose.yml", "docker-compose.yml"]
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
    else {
        return Ok(());
    };
    let mut config: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(source).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    for kind in ["volumes", "networks"] {
        if let Some(resources) = config.get(kind).and_then(serde_yaml::Value::as_mapping) {
            for (_, resource) in resources {
                if resource.get("external").is_some() || resource.get("name").is_some() {
                    return Err(format!("Compose usa {kind} externos ou com nome fixo. Use recursos reservados ao projeto do preview."));
                }
            }
        }
    }
    let services = config
        .get_mut("services")
        .and_then(serde_yaml::Value::as_mapping_mut)
        .ok_or("Compose não declara services")?;
    for (name, service) in services.iter_mut() {
        let service = service.as_mapping_mut().ok_or("Serviço Compose inválido")?;
        for prohibited in [
            "privileged",
            "network_mode",
            "pid",
            "devices",
            "container_name",
        ] {
            if service.contains_key(serde_yaml::Value::String(prohibited.into())) {
                return Err(format!(
                    "Serviço {} usa {prohibited}, incompatível com isolamento automático.",
                    name.as_str().unwrap_or("desconhecido")
                ));
            }
        }
        if let Some(volumes) = service
            .get(serde_yaml::Value::String("volumes".into()))
            .and_then(serde_yaml::Value::as_sequence)
        {
            for volume in volumes {
                let text = volume
                    .as_str()
                    .ok_or("Volumes Compose em formato longo exigem revisão")?;
                let host = text.split(':').next().unwrap_or("");
                if host.starts_with('/')
                    || host.starts_with('.')
                    || text.contains("\\")
                    || text.contains("docker.sock")
                {
                    return Err(format!("Volume {host} acessa arquivos do host. Use volume nomeado reservado por preview."));
                }
            }
        }
        if let Some(ports) = service
            .get_mut(serde_yaml::Value::String("ports".into()))
            .and_then(serde_yaml::Value::as_sequence_mut)
        {
            for port in ports {
                let text = port
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| port.as_u64().map(|p| p.to_string()))
                    .ok_or("Portas Compose em formato longo exigem revisão")?;
                let target = text.rsplit(':').next().ok_or("Porta Compose inválida")?;
                if !target
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '/' | 't' | 'c' | 'p' | 'u' | 'd'))
                {
                    return Err("Porta Compose dinâmica não suportada".into());
                }
                *port = serde_yaml::Value::String(format!("127.0.0.1::{target}"));
            }
        }
    }
    fs::write(
        root.join(".localdeploy-compose.yml"),
        serde_yaml::to_string(&config).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let project = image(root)?.replace(':', "-");
    run(
        "docker",
        &[
            "compose",
            "--project-name",
            &project,
            "--file",
            ".localdeploy-compose.yml",
            "up",
            "--detach",
            "--wait",
            "--wait-timeout",
            "120",
        ],
        root,
        log,
    )
}

pub fn cleanup(root: &Path) -> Result<(), String> {
    let project = image(root)?.replace(':', "-");
    let _ = hidden_command("docker")
        .args(["rm", "--force", &format!("{project}-preview")])
        .output();
    if root.join(".localdeploy-compose.yml").exists() {
        let output = hidden_command("docker")
            .args([
                "compose",
                "--project-name",
                &project,
                "--file",
                ".localdeploy-compose.yml",
                "down",
                "--volumes",
            ])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("Não foi possível limpar os serviços deste preview. Inicie Docker Desktop e tente novamente.".into());
        }
    }
    let _ = hidden_command("docker")
        .args(["image", "rm", &image(root)?])
        .output();
    Ok(())
}

pub fn stop(root: &Path) {
    if let Ok(image) = image(root) {
        let _ = hidden_command("docker")
            .args(["stop", &format!("{}-preview", image.replace(':', "-"))])
            .output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires Docker Linux and network access"]
    fn container_build_and_localhost_preview() {
        use std::io::{Read, Write};
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), r#"{"name":"localdeploy-fixture","version":"1.0.0","scripts":{"build":"node -e \"console.log('build ok')\"","test":"node -e \"console.log('test ok')\"","start":"node server.js"}}"#).unwrap();
        fs::write(dir.path().join("server.js"), "require('http').createServer((q,r)=>r.end('isolated-preview')).listen(3000,'0.0.0.0');").unwrap();
        let mut log = File::create(dir.path().join("build.log")).unwrap();
        build(dir.path(), true, &mut log).unwrap_or_else(|e| {
            panic!(
                "{e}: {}",
                fs::read_to_string(dir.path().join("build.log")).unwrap()
            )
        });
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let mut child = start(dir.path(), port, &log).unwrap();
        let mut response = String::new();
        for _ in 0..600 {
            if let Ok(mut stream) = std::net::TcpStream::connect_timeout(
                &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
                std::time::Duration::from_millis(200),
            ) {
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let _ = stream.write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n");
                let _ = stream.read_to_string(&mut response);
                if response.contains("isolated-preview") {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        let _ = child.kill();
        cleanup(dir.path()).unwrap();
        let _ = child.wait();
        assert!(response.contains("isolated-preview"));
    }
    #[test]
    fn desktop_and_web_require_explicit_platform() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Dioxus.toml"),
            "[application]\ndefault_platform = 'desktop'\n",
        )
        .unwrap();
        assert_eq!(detect(dir.path()).unwrap(), Backend::DioxusDesktop);
        fs::write(
            dir.path().join("Dioxus.toml"),
            "[application]\ndefault_platform = 'web'\n",
        )
        .unwrap();
        assert_eq!(detect(dir.path()).unwrap(), Backend::DioxusWeb);
        fs::write(dir.path().join("Dioxus.toml"), "[application]\n").unwrap();
        assert!(detect(dir.path()).unwrap_err().contains("default_platform"));
    }
    #[test]
    fn unknown_stack_is_blocked_without_executing_code() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect(dir.path()).is_err());
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(detect(dir.path()).unwrap(), Backend::RustWeb);
    }
}
