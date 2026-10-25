use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::detector::StackInfo;
use crate::gitops;
use crate::process::hidden_command;
use crate::storage::{self, BuildRecord, NewBuild, Project};

const PREVIEW_WAIT: Duration = Duration::from_secs(120);

pub fn cleanup_ephemeral_workspaces() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetTickCount64() -> u64;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let boot = now.saturating_sub(unsafe { GetTickCount64() } / 1000);
        let marker = storage::app_data_dir()
            .map_err(|e| e.to_string())?
            .join("last-boot");
        let previous = fs::read_to_string(&marker)
            .ok()
            .and_then(|s| s.parse::<u64>().ok());
        if previous.is_some_and(|previous| previous.abs_diff(boot) < 120) {
            return Ok(());
        }
        fs::write(&marker, boot.to_string()).map_err(|e| e.to_string())?;
    }
    for project in storage::list_projects().map_err(|error| error.to_string())? {
        let protected_workspace = storage::active_production(project.id)
            .map_err(|error| error.to_string())?
            .map(|build| build.workspace_path);
        for build in storage::list_builds(project.id).map_err(|error| error.to_string())? {
            if build.environment != "preview"
                || protected_workspace.as_deref() == Some(build.workspace_path.as_str())
            {
                continue;
            }
            let workspace = Path::new(&build.workspace_path);
            crate::environments::cleanup(workspace)?;
            match fs::remove_dir_all(workspace) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "Não foi possível limpar {}: {error}",
                        workspace.display()
                    ));
                }
            }
            storage::update_build(build.id, "expired", None, None, None, true)
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub fn delete_preview_workspace(project: &Project, build: &BuildRecord) -> Result<(), String> {
    if build.project_id != project.id || build.environment != "preview" {
        return Err("Este registro não é um workspace de preview deste projeto.".into());
    }
    if storage::active_production(project.id)
        .map_err(|error| error.to_string())?
        .is_some_and(|production| production.workspace_path == build.workspace_path)
    {
        return Err("Este workspace está em produção local e não pode ser apagado.".into());
    }
    if let Some(process_id) = build.process_id {
        let _ = stop_process(process_id as u32);
    }
    crate::environments::cleanup(Path::new(&build.workspace_path))?;
    gitops::remove_worktree(
        Path::new(&project.local_path),
        Path::new(&build.workspace_path),
    )?;
    storage::update_build(build.id, "expired", None, None, None, true)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionPlan {
    pub commit_sha: String,
    pub workspace: PathBuf,
    pub package_manager: String,
    pub install_action: String,
    pub build_action: String,
    pub test_action: String,
    pub preview_action: String,
    pub warnings: Vec<String>,
}

pub fn plan(project: &Project, reference: &str) -> Result<ExecutionPlan, String> {
    let repository = Path::new(&project.local_path);
    let commit_sha = if reference.len() == 40
        && reference
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        reference.to_ascii_lowercase()
    } else {
        gitops::resolve_commit(repository, reference)?
    };
    // This analysis only describes the preparation dialog. The executor repeats
    // the analysis after checking out the selected SHA, because package.json,
    // lockfiles and scripts may be different in every commit.
    let info = crate::detector::analyze(repository);
    let backend = crate::environments::detect(repository)?;
    if backend != crate::environments::Backend::Node
        || (!repository.join("localdeploy-native").exists()
            && crate::preferences::load().backend == "Docker")
    {
        return Ok(ExecutionPlan {
            workspace: workspace_path(project.id, &commit_sha)?, commit_sha,
            package_manager: format!("{backend:?}"),
            install_action: "Preparar ferramentas no ambiente reservado".into(),
            build_action: "Build isolado do snapshot selecionado".into(),
            test_action: "Executar testes declarados".into(),
            preview_action: if backend == crate::environments::Backend::DioxusDesktop { "Abrir janela nativa".into() } else { "Publicar somente em localhost".into() },
            warnings: vec!["Aplicações web isoladas exigem Docker Desktop com contêineres Linux; desktop exige Rust e linker nativos.".into()],
        });
    }
    let manager = manager(&info)?;
    let workspace = workspace_path(project.id, &commit_sha)?;
    let install_action = match manager.as_str() {
        "npm" if info.has_lockfile => "npm ci".into(),
        "npm" => "npm install (sem lockfile)".into(),
        "pnpm" => "pnpm install --frozen-lockfile".into(),
        "yarn" => "yarn install --immutable".into(),
        "bun" => "bun install --frozen-lockfile".into(),
        _ => unreachable!(),
    };
    let build_action = info
        .build_script
        .as_ref()
        .map(|_| format!("{manager} run build"))
        .ok_or("O projeto não declara o script build no package.json.")?;
    let preview_action = if info.technologies.iter().any(|item| item == "Vite") {
        "servir dist compilado em uma porta local livre".into()
    } else if info.technologies.iter().any(|item| item == "Next.js") || info.start_script.is_some()
    {
        format!("{manager} run start em uma porta local livre")
    } else {
        "servir o artefato estático produzido pelo build".into()
    };
    let test_action = if info.test_script.is_some() {
        format!("{manager} run test")
    } else {
        "Nenhum script de teste configurado".into()
    };
    let mut warnings = vec![
        "Scripts de instalação, build e teste são código do repositório e só serão executados após sua aprovação.".into(),
        "O workspace é isolado por SHA e não altera a cópia principal do projeto.".into(),
        "Os manifests serão analisados novamente depois do checkout deste commit; as ações abaixo podem mudar se este SHA declarar ferramentas diferentes.".into(),
    ];
    if !info.has_lockfile {
        warnings.push("Não há lockfile: a instalação pode não ser reproduzível.".into());
    }
    if info.has_compose || info.has_supabase {
        warnings.push(
            "Serviços Docker e Supabase não são iniciados automaticamente nesta execução.".into(),
        );
    }
    Ok(ExecutionPlan {
        commit_sha,
        workspace,
        package_manager: manager,
        install_action,
        build_action,
        test_action,
        preview_action,
        warnings,
    })
}

/// Prepares the exact SHA without exposing a server. A successful record can
/// later be opened as a local preview without rebuilding it.
pub fn prepare_preview(
    project: &Project,
    reference: &str,
    run_tests: bool,
) -> Result<BuildRecord, String> {
    let plan = plan(project, reference)?;
    let log_path = log_path(project.id, &plan.commit_sha, "preview")?;
    let build = storage::create_build(NewBuild {
        project_id: project.id,
        commit_sha: &plan.commit_sha,
        environment: "preview",
        log_path: &log_path,
        workspace_path: &plan.workspace,
    })
    .map_err(|error| error.to_string())?;

    let outcome = prepare_preview_inner(project, &plan, &log_path, run_tests);
    match outcome {
        Ok(()) => {
            storage::update_build(build.id, "ready", None, None, None, false)
                .map_err(|error| error.to_string())?;
            let mut record = build;
            record.status = "ready".into();
            Ok(record)
        }
        Err(error) => {
            let status = if error.to_lowercase().contains("cancelad") {
                "cancelled"
            } else {
                "failed"
            };
            let _ = storage::update_build(build.id, status, None, None, Some(&error), true);
            Err(format!("{error} Consulte o log em {}.", log_path.display()))
        }
    }
}

fn prepare_preview_inner(
    project: &Project,
    plan: &ExecutionPlan,
    log_path: &Path,
    run_tests: bool,
) -> Result<(), String> {
    let mut log = create_log(log_path)?;
    write_line(&mut log, "LocalDeploy: preparando preview isolado.")?;
    gitops::create_worktree(
        Path::new(&project.local_path),
        &plan.commit_sha,
        &plan.workspace,
    )?;
    let _ = fs::remove_file(plan.workspace.join(".localdeploy-cancel"));
    crate::secrets::materialize(project.id, &plan.workspace)?;
    write_line(
        &mut log,
        &format!("Workspace: {}", plan.workspace.display()),
    )?;
    let info = crate::detector::analyze(&plan.workspace);
    let backend = crate::environments::detect(&plan.workspace)?;
    if backend != crate::environments::Backend::Node
        || (!plan.workspace.join("localdeploy-native").exists()
            && crate::preferences::load().backend == "Docker")
    {
        return crate::environments::build(&plan.workspace, run_tests, &mut log);
    }
    let exact_manager = manager(&info)?;
    let mut tool_paths = crate::toolchains::prepare(
        &plan.workspace,
        info.node_version.as_deref(),
        exact_manager == "bun",
    )?;
    fs::write(
        plan.workspace.join(".localdeploy-tools.json"),
        serde_json::to_vec(&tool_paths).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    crate::process::set_tool_paths(tool_paths.clone());
    write_line(
        &mut log,
        &format!("Reanálise do SHA {}: {}.", plan.commit_sha, info.summary()),
    )?;
    if ensure_command(&exact_manager).is_err() && matches!(exact_manager.as_str(), "pnpm" | "yarn")
    {
        let tools = storage::app_data_dir()
            .map_err(|e| e.to_string())?
            .join("tools")
            .join(project.id.to_string());
        fs::create_dir_all(&tools).map_err(|e| e.to_string())?;
        let prefix = tools.to_string_lossy().into_owned();
        let package = info.package_manager.as_deref().unwrap_or(&exact_manager);
        if !package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '-' | '+'))
        {
            return Err("Versão do gerenciador de pacotes inválida.".into());
        }
        run_command(
            "npm",
            &["install", "--global", "--prefix", &prefix, package],
            &plan.workspace,
            &mut log,
        )?;
        tool_paths.extend([tools.clone(), tools.join("bin")]);
        crate::process::set_tool_paths(tool_paths.clone());
        fs::write(
            plan.workspace.join(".localdeploy-tools.json"),
            serde_json::to_vec(&tool_paths).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    ensure_command(&exact_manager)?;
    run_install(&exact_manager, &info, &plan.workspace, &mut log)?;
    run_script(&exact_manager, "build", &plan.workspace, &mut log)?;
    if run_tests && info.test_script.is_some() {
        run_script(&exact_manager, "test", &plan.workspace, &mut log)?;
    } else if info.test_script.is_none() {
        write_line(&mut log, "Testes: não configurados pelo projeto.")?;
    } else {
        write_line(&mut log, "Testes: ignorados por escolha do usuário.")?;
    }
    write_line(&mut log, "Build aprovado. O preview local pode ser aberto.")?;
    Ok(())
}

/// Starts the already approved artifact of a build. No install, test or build
/// command is repeated here.
pub fn start_preview(project: &Project, build: &BuildRecord) -> Result<BuildRecord, String> {
    if build.project_id != project.id || build.environment != "preview" || build.status != "ready" {
        return Err("Somente um build de preview aprovado pode ser aberto.".into());
    }
    if let Some(endpoint) = build.endpoint.as_ref() {
        if build.process_id.is_some()
            && wait_for_health(endpoint, Duration::from_millis(100)).is_ok()
        {
            return Ok(build.clone());
        }
    }
    let workspace = Path::new(&build.workspace_path);
    if !workspace.is_dir() {
        return Err(
            "O workspace deste build não está mais disponível. Prepare o commit novamente.".into(),
        );
    }
    let info = crate::detector::analyze(workspace);
    if let Ok(bytes) = fs::read(workspace.join(".localdeploy-tools.json")) {
        crate::process::set_tool_paths(serde_json::from_slice(&bytes).map_err(|e| e.to_string())?);
    }
    let package_manager = manager(&info)?;
    let mut log = OpenOptions::new()
        .append(true)
        .open(&build.log_path)
        .map_err(|error| format!("Não foi possível reabrir o log do build: {error}"))?;
    let port = reserve_port()?;
    let mut child = start_preview_server(&package_manager, &info, workspace, port, &mut log)?;
    if crate::environments::detect(workspace)? == crate::environments::Backend::DioxusDesktop {
        thread::sleep(Duration::from_millis(500));
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!(
                "Aplicação desktop encerrou ao iniciar: {status}. Consulte o log."
            ));
        }
        storage::update_build(build.id, "ready", None, Some(child.id()), None, false)
            .map_err(|e| e.to_string())?;
        let mut record = build.clone();
        record.process_id = Some(i64::from(child.id()));
        record.endpoint = None;
        return Ok(record);
    }
    let endpoint = format!("http://127.0.0.1:{port}");
    if let Err(error) = wait_for_health(&endpoint, PREVIEW_WAIT) {
        let _ = child.kill();
        let _ = child.wait();
        crate::environments::stop(workspace);
        return Err(error);
    }
    write_line(&mut log, &format!("Preview aberto: {endpoint}"))?;
    storage::update_build(
        build.id,
        "ready",
        Some(&endpoint),
        Some(child.id()),
        None,
        false,
    )
    .map_err(|error| error.to_string())?;
    let mut record = build.clone();
    record.endpoint = Some(endpoint);
    record.process_id = Some(i64::from(child.id()));
    Ok(record)
}

pub fn stop_preview(build: &BuildRecord) -> Result<(), String> {
    crate::environments::stop(Path::new(&build.workspace_path));
    let process_id = build
        .process_id
        .ok_or("Este preview não possui processo registrado.")? as u32;
    let workspace = Path::new(&build.workspace_path);
    if workspace.join("localdeploy-native").exists()
        || crate::environments::detect(workspace).ok()
            == Some(crate::environments::Backend::DioxusDesktop)
    {
        stop_process(process_id)?;
    } else {
        let _ = stop_process(process_id);
    }
    storage::update_build(
        build.id,
        "stopped",
        build.endpoint.as_deref(),
        Some(process_id),
        None,
        true,
    )
    .map_err(|error| error.to_string())
}

pub fn promote_preview(project: &Project, source: &BuildRecord) -> Result<BuildRecord, String> {
    if source.project_id != project.id || source.status != "ready" {
        return Err("Somente um preview saudável deste projeto pode ser promovido.".into());
    }
    let workspace = PathBuf::from(&source.workspace_path);
    if !workspace.is_dir() {
        return Err("O workspace deste preview não está mais disponível para promoção.".into());
    }
    let log_path = log_path(project.id, &source.commit_sha, "production")?;
    let production = storage::create_build(NewBuild {
        project_id: project.id,
        commit_sha: &source.commit_sha,
        environment: "production",
        log_path: &log_path,
        workspace_path: &workspace,
    })
    .map_err(|error| error.to_string())?;
    let result = promote_preview_inner(project, source, &workspace, &log_path);
    match result {
        Ok((endpoint, process_id)) => {
            storage::update_build(
                production.id,
                "ready",
                Some(&endpoint),
                Some(process_id),
                None,
                false,
            )
            .map_err(|error| error.to_string())?;
            storage::promote_build(project.id, production.id, &endpoint)
                .map_err(|error| error.to_string())?;
            let mut record = production;
            record.status = "ready".into();
            record.endpoint = Some(endpoint);
            record.process_id = Some(i64::from(process_id));
            Ok(record)
        }
        Err(error) => {
            let _ = storage::update_build(production.id, "failed", None, None, Some(&error), true);
            Err(format!(
                "A promoção falhou: {error} Consulte o log em {}.",
                log_path.display()
            ))
        }
    }
}

pub fn rollback_production(project: &Project) -> Result<BuildRecord, String> {
    let active = storage::active_production(project.id)
        .map_err(|error| error.to_string())?
        .ok_or("Não existe produção local ativa para reverter.")?;
    let candidates = storage::list_builds(project.id).map_err(|error| error.to_string())?;
    let previous = candidates
        .into_iter()
        .find(|build| {
            build.id != active.id
                && build.environment == "production"
                && matches!(build.status.as_str(), "ready" | "stopped")
        })
        .ok_or("Não há uma produção anterior saudável para rollback.")?;
    if let Some(process_id) = active.process_id {
        let _ = stop_process(process_id as u32);
        let _ = storage::update_build(
            active.id,
            "stopped",
            active.endpoint.as_deref(),
            Some(process_id as u32),
            None,
            true,
        );
    }
    restart_production(project, &previous)
}

fn promote_preview_inner(
    project: &Project,
    source: &BuildRecord,
    workspace: &Path,
    log_path: &Path,
) -> Result<(String, u32), String> {
    let mut log = create_log(log_path)?;
    let info = crate::detector::analyze(workspace);
    write_line(
        &mut log,
        &format!("Promovendo preview {} para produção local.", source.id),
    )?;

    // Validate the exact artifact on an unused loopback port before touching production.
    let candidate_port = reserve_port()?;
    let mut candidate = start_production_server(workspace, &info, candidate_port, &mut log)?;
    let candidate_endpoint = format!("http://127.0.0.1:{candidate_port}");
    if let Err(error) = wait_for_health(&candidate_endpoint, PREVIEW_WAIT) {
        let _ = candidate.kill();
        let _ = candidate.wait();
        return Err(format!(
            "A versão candidata não passou no health check: {error}"
        ));
    }
    let _ = candidate.kill();
    let _ = candidate.wait();

    let previous = storage::active_production(project.id).map_err(|error| error.to_string())?;
    if let Some(previous) = previous.as_ref() {
        if let Some(process_id) = previous.process_id {
            stop_process(process_id as u32)?;
            storage::update_build(
                previous.id,
                "stopped",
                previous.endpoint.as_deref(),
                Some(process_id as u32),
                None,
                true,
            )
            .map_err(|error| error.to_string())?;
        }
    }

    match start_production_server(workspace, &info, 4000, &mut log) {
        Ok(mut child) => {
            let endpoint = "http://127.0.0.1:4000".to_string();
            if let Err(error) = wait_for_health(&endpoint, PREVIEW_WAIT) {
                let _ = child.kill();
                let _ = child.wait();
                if let Some(previous) = previous.as_ref() {
                    let _ = restart_production(project, previous);
                }
                return Err(format!(
                    "A nova produção não passou no health check: {error}"
                ));
            }
            write_line(&mut log, &format!("Produção local ativa: {endpoint}"))?;
            Ok((endpoint, child.id()))
        }
        Err(error) => {
            if let Some(previous) = previous.as_ref() {
                let _ = restart_production(project, previous);
            }
            Err(error)
        }
    }
}

fn restart_production(project: &Project, build: &BuildRecord) -> Result<BuildRecord, String> {
    let workspace = Path::new(&build.workspace_path);
    let info = crate::detector::analyze(workspace);
    let mut log = OpenOptions::new()
        .append(true)
        .open(&build.log_path)
        .map_err(|error| format!("Não foi possível reabrir o log de rollback: {error}"))?;
    let mut child = start_production_server(workspace, &info, 4000, &mut log)?;
    let endpoint = "http://127.0.0.1:4000";
    if let Err(error) = wait_for_health(endpoint, PREVIEW_WAIT) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("O rollback não passou no health check: {error}"));
    }
    storage::update_build(
        build.id,
        "ready",
        Some(endpoint),
        Some(child.id()),
        None,
        false,
    )
    .map_err(|error| error.to_string())?;
    storage::promote_build(project.id, build.id, endpoint).map_err(|error| error.to_string())?;
    let mut record = build.clone();
    record.status = "ready".into();
    record.endpoint = Some(endpoint.into());
    record.process_id = Some(i64::from(child.id()));
    Ok(record)
}

pub fn read_log(build: &BuildRecord) -> Result<String, String> {
    let mut content = String::new();
    File::open(&build.log_path)
        .map_err(|error| format!("Não foi possível abrir o log: {error}"))?
        .read_to_string(&mut content)
        .map_err(|error| format!("Não foi possível ler o log: {error}"))?;
    Ok(sanitize_log(&content))
}

fn manager(info: &StackInfo) -> Result<String, String> {
    let manager = info
        .package_manager
        .as_deref()
        .unwrap_or("npm")
        .split('@')
        .next()
        .unwrap_or("npm");
    match manager {
        "npm" | "pnpm" | "yarn" | "bun" => Ok(manager.into()),
        _ => Err("O gerenciador de pacotes declarado não é suportado para preview.".into()),
    }
}

fn workspace_path(project_id: i64, sha: &str) -> Result<PathBuf, String> {
    let root = storage::app_data_dir().map_err(|error| error.to_string())?;
    Ok(root
        .join("workspaces")
        .join(project_id.to_string())
        .join(sha))
}

fn log_path(project_id: i64, sha: &str, environment: &str) -> Result<PathBuf, String> {
    let root = storage::app_data_dir().map_err(|error| error.to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    Ok(root
        .join("logs")
        .join(project_id.to_string())
        .join(format!("{environment}-{sha}-{nonce}.log")))
}

fn create_log(path: &Path) -> Result<File, String> {
    let parent = path
        .parent()
        .ok_or("O caminho de log não possui pasta pai.")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Não foi possível criar a pasta de logs: {error}"))?;
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("Não foi possível criar o log: {error}"))
}

fn write_line(log: &mut File, message: &str) -> Result<(), String> {
    writeln!(log, "{message}").map_err(|error| format!("Não foi possível gravar o log: {error}"))
}

fn ensure_command(command: &str) -> Result<(), String> {
    hidden_command(command)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| format!("{command} não foi encontrado no PATH. Instale o runtime necessário e tente novamente."))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("{command} não respondeu corretamente a --version."))
            }
        })
}

fn run_install(manager: &str, info: &StackInfo, cwd: &Path, log: &mut File) -> Result<(), String> {
    let args: Vec<&str> = match manager {
        "npm" if info.has_lockfile => vec!["ci"],
        "npm" => vec!["install"],
        "pnpm" => vec!["install", "--frozen-lockfile"],
        "yarn" => vec!["install", "--immutable"],
        "bun" => vec!["install", "--frozen-lockfile"],
        _ => return Err("Gerenciador de pacotes não suportado.".into()),
    };
    run_command(manager, &args, cwd, log)
}

fn run_script(manager: &str, script: &str, cwd: &Path, log: &mut File) -> Result<(), String> {
    let args = match manager {
        "npm" | "pnpm" | "bun" => vec!["run", script],
        "yarn" => vec![script],
        _ => return Err("Gerenciador de pacotes não suportado.".into()),
    };
    run_command(manager, &args, cwd, log)
}

pub(crate) fn run_command(
    command: &str,
    args: &[&str],
    cwd: &Path,
    log: &mut File,
) -> Result<(), String> {
    if cwd.join(".localdeploy-cancel").exists() {
        return Err("Build cancelado pelo usuário.".into());
    }
    fs::write(
        cwd.join(".localdeploy-progress"),
        format!("{command} {}", args.join(" ")),
    )
    .map_err(|error| error.to_string())?;
    write_line(log, &format!("> {command} {}", args.join(" ")))?;
    let stderr = log.try_clone().map_err(|error| error.to_string())?;
    let mut child = hidden_command(command)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone().map_err(|error| error.to_string())?,
        ))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("Não foi possível iniciar {command}: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(1800);
    let status = loop {
        if cwd.join(".localdeploy-cancel").exists() || Instant::now() >= deadline {
            let _ = stop_process(child.id());
            let _ = child.kill();
            let _ = child.wait();
            return Err("Build cancelado ou excedeu o limite de 30 minutos por etapa.".into());
        }
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        thread::sleep(Duration::from_millis(200));
    };
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{command} terminou com código {:?}.",
            status.code()
        ))
    }
}

fn start_preview_server(
    manager: &str,
    info: &StackInfo,
    cwd: &Path,
    port: u16,
    log: &mut File,
) -> Result<Child, String> {
    if crate::environments::detect(cwd)? != crate::environments::Backend::Node
        || (!cwd.join("localdeploy-native").exists()
            && crate::preferences::load().backend == "Docker")
    {
        return crate::environments::start(cwd, port, log);
    }
    if info
        .technologies
        .iter()
        .any(|technology| technology == "Vite")
    {
        return start_static_server(cwd, "dist", port, log);
    }
    if info
        .technologies
        .iter()
        .any(|technology| technology == "Next.js")
    {
        return start_node_server(manager, info, cwd, port, log, "start");
    }
    for directory in ["dist", "build", "out", "public"] {
        if cwd.join(directory).join("index.html").is_file() {
            return start_static_server(cwd, directory, port, log);
        }
    }
    if info.start_script.is_some() {
        return start_node_server(manager, info, cwd, port, log, "start");
    }
    Err("O build não produziu um site estático conhecido e o projeto não declara o script start para servir o artefato.".into())
}

fn start_node_server(
    manager: &str,
    info: &StackInfo,
    cwd: &Path,
    port: u16,
    log: &mut File,
    script: &str,
) -> Result<Child, String> {
    let port_text = port.to_string();
    let mut args: Vec<&str> = match manager {
        "npm" | "pnpm" | "bun" => vec!["run", script, "--"],
        "yarn" => vec![script, "--"],
        _ => return Err("Gerenciador de pacotes não suportado.".into()),
    };
    if info
        .technologies
        .iter()
        .any(|technology| technology == "Next.js")
    {
        args.extend(["--hostname", "127.0.0.1", "--port", &port_text]);
    } else {
        args.extend(["--host", "127.0.0.1", "--port", &port_text, "--strictPort"]);
    }
    write_line(log, &format!("> {manager} {}", args.join(" ")))?;
    let stdout = log.try_clone().map_err(|error| error.to_string())?;
    let stderr = log.try_clone().map_err(|error| error.to_string())?;
    hidden_command(manager)
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("Não foi possível iniciar o servidor do artefato: {error}"))
}

fn start_static_server(
    workspace: &Path,
    artifact_directory: &str,
    port: u16,
    log: &mut File,
) -> Result<Child, String> {
    let artifact = workspace.join(artifact_directory);
    if !artifact.join("index.html").is_file() {
        return Err(format!(
            "O build não gerou {}/index.html para servir o preview.",
            artifact_directory
        ));
    }
    let executable = std::env::current_exe().map_err(|error| {
        format!("Não foi possível localizar o executável do LocalDeploy: {error}")
    })?;
    let port_text = port.to_string();
    write_line(
        log,
        &format!(
            "> LocalDeploy --serve-directory {} --port {port}",
            artifact.display()
        ),
    )?;
    let stdout = log.try_clone().map_err(|error| error.to_string())?;
    let stderr = log.try_clone().map_err(|error| error.to_string())?;
    hidden_command(executable)
        .args([
            "--serve-directory",
            &artifact.to_string_lossy(),
            "--port",
            &port_text,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("Não foi possível iniciar o servidor estático: {error}"))
}

fn start_production_server(
    workspace: &Path,
    info: &StackInfo,
    port: u16,
    log: &mut File,
) -> Result<Child, String> {
    if let Ok(bytes) = fs::read(workspace.join(".localdeploy-tools.json")) {
        crate::process::set_tool_paths(serde_json::from_slice(&bytes).map_err(|e| e.to_string())?);
    }
    let port_text = port.to_string();
    let stdout = log.try_clone().map_err(|error| error.to_string())?;
    let stderr = log.try_clone().map_err(|error| error.to_string())?;
    if info
        .technologies
        .iter()
        .any(|technology| technology == "Vite")
    {
        return start_static_server(workspace, "dist", port, log);
    }
    let manager = manager(info)?;
    if info.start_script.is_none() {
        return Err("O projeto não declara um script start para produção local.".into());
    }
    let mut args: Vec<&str> = match manager.as_str() {
        "npm" | "pnpm" | "bun" => vec!["run", "start", "--"],
        "yarn" => vec!["start", "--"],
        _ => return Err("Gerenciador de pacotes não suportado.".into()),
    };
    if info
        .technologies
        .iter()
        .any(|technology| technology == "Next.js")
    {
        args.extend(["--hostname", "127.0.0.1", "--port", &port_text]);
    } else {
        args.extend(["--host", "127.0.0.1", "--port", &port_text]);
    }
    write_line(log, &format!("> {manager} {}", args.join(" ")))?;
    hidden_command(manager)
        .args(&args)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("Não foi possível iniciar produção local: {error}"))
}

fn reserve_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("Não foi possível reservar uma porta local: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| error.to_string())
}

fn wait_for_health(endpoint: &str, timeout: Duration) -> Result<(), String> {
    let address: SocketAddr = endpoint
        .strip_prefix("http://")
        .ok_or("Endpoint de preview inválido.")?
        .parse()
        .map_err(|_| "Endpoint de preview inválido.")?;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_secs(1)) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            if stream
                .write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")
                .is_ok()
            {
                let mut response = [0_u8; 32];
                if let Ok(read) = stream.read(&mut response) {
                    let header = String::from_utf8_lossy(&response[..read]);
                    if header.starts_with("HTTP/1.") && !header.contains(" 5") {
                        return Ok(());
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(400));
    }
    Err("O servidor não respondeu ao health check. Confira o log, a porta 3000 e o bind 0.0.0.0 no contêiner.".into())
}

fn stop_process(process_id: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let status = hidden_command("taskkill")
        .args(["/PID", &process_id.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(not(target_os = "windows"))]
    let status = hidden_command("kill")
        .args(["-TERM", "--", &format!("-{process_id}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err("Não foi possível encerrar o processo de preview.".into()),
        Err(error) => Err(format!(
            "Não foi possível encerrar o processo de preview: {error}"
        )),
    }
}

fn sanitize_log(content: &str) -> String {
    content
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.contains("token=")
                || lower.contains("password=")
                || lower.contains("authorization:")
            {
                "[linha ocultada por possível segredo]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::NewProject;

    #[test]
    fn removes_possible_secret_lines_from_log() {
        assert_eq!(
            sanitize_log("ok\nAuthorization: Bearer secret\nfinal"),
            "ok\n[linha ocultada por possível segredo]\nfinal"
        );
    }

    #[test]
    #[ignore = "requires Node.js and npm on PATH"]
    fn preview_pipeline_uses_an_isolated_worktree_and_health_check() {
        let temporary = tempfile::tempdir().unwrap();
        let data_dir = temporary.path().join("data");
        std::env::set_var("LOCALDEPLOY_DATA_DIR", &data_dir);
        let repository = temporary.path().join("repository");
        fs::create_dir_all(&repository).unwrap();
        fs::write(
            repository.join("package.json"),
            r#"{"name":"fixture","version":"1.0.0","scripts":{"build":"node -e \"require('fs').mkdirSync('dist',{recursive:true});require('fs').writeFileSync('dist/index.html','fixture')\"","dev":"node server.js"}}"#,
        )
        .unwrap();
        fs::write(
            repository.join("package-lock.json"),
            r#"{"name":"fixture","version":"1.0.0","lockfileVersion":3,"requires":true,"packages":{"":{"name":"fixture","version":"1.0.0"}}}"#,
        )
        .unwrap();
        fs::write(
            repository.join("server.js"),
            r#"const http=require('http');const i=process.argv.indexOf('--port');const port=Number(i >= 0 ? process.argv[i+1] : 3000);http.createServer((_,res)=>res.end('ok')).listen(port,'127.0.0.1');"#,
        )
        .unwrap();
        run_git(&repository, &["init", "-b", "main"]);
        run_git(&repository, &["add", "."]);
        run_git(
            &repository,
            &[
                "-c",
                "user.name=LocalDeploy",
                "-c",
                "user.email=local@example.test",
                "commit",
                "-m",
                "fixture",
            ],
        );
        let project = storage::add_project(
            NewProject::new(
                "https://github.com/acme/fixture",
                "main",
                repository.clone(),
            )
            .unwrap(),
        )
        .unwrap();
        let prepared = prepare_preview(&project, "HEAD", false).unwrap();
        let build = start_preview(&project, &prepared).unwrap();
        assert_eq!(build.status, "ready");
        assert!(build
            .endpoint
            .as_deref()
            .unwrap()
            .starts_with("http://127.0.0.1:"));
        stop_preview(&build).unwrap();
        std::env::remove_var("LOCALDEPLOY_DATA_DIR");
    }

    fn run_git(directory: &Path, arguments: &[&str]) {
        let status = hidden_command("git")
            .arg("-C")
            .arg(directory)
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success());
    }
}
