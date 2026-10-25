use std::fs;
#[cfg(target_os = "windows")]
use std::fs::File;
#[cfg(target_os = "windows")]
use std::io::{self, Write};
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::process::Stdio;

#[cfg(target_os = "windows")]
use reqwest::blocking::Client;
#[cfg(target_os = "windows")]
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde_json::Value;
#[cfg(target_os = "windows")]
use zip::ZipArchive;

#[cfg(target_os = "windows")]
use crate::process::hidden_command;
use crate::storage::{self, Project};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerStatus {
    NotInstalled,
    Registered,
    Running,
}

#[derive(Debug, Clone)]
pub struct RunnerInfo {
    pub status: RunnerStatus,
    pub directory: PathBuf,
    pub name: Option<String>,
}

pub fn inspect(project: &Project) -> Result<RunnerInfo, String> {
    let directory = runner_directory(project)?;
    let registered = directory.join(".runner").is_file();
    let name = fs::read_to_string(directory.join(".runner"))
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|value| {
            value
                .get("agentName")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let status = match read_pid(&directory).filter(|pid| process_is_running(*pid)) {
        Some(_) => RunnerStatus::Running,
        None if registered => RunnerStatus::Registered,
        None => RunnerStatus::NotInstalled,
    };
    Ok(RunnerInfo {
        status,
        directory,
        name,
    })
}

pub fn generate_workflow(project: &Project) -> Result<(), String> {
    let root = Path::new(&project.local_path).join(".github/workflows");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("localdeploy.yml");
    if path.exists() {
        return Err("O workflow localdeploy.yml já existe. Revise-o antes de substituir.".into());
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let executable = executable.to_string_lossy().replace('\'', "''");
    let workflow = format!("name: LocalDeploy\non:\n  push:\n  workflow_dispatch:\npermissions:\n  contents: read\nconcurrency:\n  group: localdeploy-project-{}\n  cancel-in-progress: false\njobs:\n  preview:\n    runs-on: [self-hosted, Windows, X64, localdeploy]\n    steps:\n      - name: Build e testes do commit\n        shell: pwsh\n        env:\n          COMMIT_SHA: ${{{{ github.sha }}}}\n        run: |\n          $process = Start-Process -FilePath '{}' -ArgumentList @('--build-commit', '{}', $env:COMMIT_SHA) -Wait -PassThru -NoNewWindow\n          if ($process.ExitCode -ne 0) {{ exit $process.ExitCode }}\n", project.id, executable, project.id);
    let workflow = workflow.replace(
        "Windows, X64, localdeploy]",
        &format!("Windows, X64, localdeploy-project-{}]", project.id),
    );
    fs::write(path, workflow).map_err(|e| e.to_string())
}

/// Adapt only compatible Windows jobs; preserve Linux/container jobs and make
/// a backup before every change. Parsing YAML avoids altering strings in steps.
pub fn adapt_workflows(project: &Project) -> Result<String, String> {
    let root = Path::new(&project.local_path).join(".github/workflows");
    let mut changed = 0;
    let mut blocked = Vec::new();
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("yaml" | "yml")
        ) {
            continue;
        }
        let original = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut document: serde_yaml::Value =
            serde_yaml::from_str(&original).map_err(|e| format!("{}: {e}", path.display()))?;
        let Some(jobs) = document
            .get_mut("jobs")
            .and_then(serde_yaml::Value::as_mapping_mut)
        else {
            continue;
        };
        let mut modified = false;
        for (name, job) in jobs.iter_mut() {
            let Some(job) = job.as_mapping_mut() else {
                continue;
            };
            let key = serde_yaml::Value::String("runs-on".into());
            let windows = job
                .get(&key)
                .and_then(serde_yaml::Value::as_str)
                .is_some_and(|value| value.starts_with("windows-"));
            if !windows
                || job.contains_key(serde_yaml::Value::String("container".into()))
                || job.contains_key(serde_yaml::Value::String("services".into()))
            {
                blocked.push(format!(
                    "{} / {}: mantido por exigir revisão de compatibilidade",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    name.as_str().unwrap_or("job")
                ));
                continue;
            }
            job.insert(
                key,
                serde_yaml::to_value(vec![
                    "self-hosted".to_string(),
                    "Windows".into(),
                    "X64".into(),
                    format!("localdeploy-project-{}", project.id),
                ])
                .map_err(|e| e.to_string())?,
            );
            modified = true;
        }
        if modified {
            let backup = path.with_extension("localdeploy-backup");
            if backup.exists() {
                return Err(format!(
                    "Backup já existe: {}. Revise antes de repetir.",
                    backup.display()
                ));
            }
            fs::write(&backup, original).map_err(|e| e.to_string())?;
            fs::write(
                &path,
                serde_yaml::to_string(&document).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            changed += 1;
        }
    }
    Ok(format!(
        "{changed} workflows adaptados com backup. Faça commit e push para ativar. {}",
        blocked.join("; ")
    ))
}

/// Downloads the runner published by GitHub, registers it for this repository,
/// then starts it. The personal token is only used for this request and is never
/// persisted. GitHub's short lived registration token is passed directly to
/// config.cmd and is also never written to disk by LocalDeploy.
pub fn install_register_and_start(
    project: &Project,
    personal_token: &str,
) -> Result<RunnerInfo, String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (project, personal_token);
        Err("O runner self-hosted é disponibilizado pelo LocalDeploy somente no Windows.".into())
    }

    #[cfg(target_os = "windows")]
    {
        let token = personal_token.trim();
        if token.is_empty() {
            return Err("Informe um token de acesso do GitHub para registrar o runner.".into());
        }
        let directory = runner_directory(project)?;
        let client = github_client()?;
        let registration_token = request_registration_token(&client, project, token)?;
        if !directory.join("run.cmd").is_file() {
            download_runner(&client, &directory)?;
        }
        if !directory.join(".runner").is_file() {
            configure_runner(project, &directory, &registration_token)?;
        }
        start(&directory)?;
        inspect(project)
    }
}

pub fn start(directory: &Path) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = directory;
        Err("O runner self-hosted é disponibilizado pelo LocalDeploy somente no Windows.".into())
    }
    #[cfg(target_os = "windows")]
    {
        if read_pid(directory)
            .filter(|pid| process_is_running(*pid))
            .is_some()
        {
            return Ok(());
        }
        let child = hidden_command("cmd")
            .args(["/C", "run.cmd"])
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("Não foi possível iniciar o runner: {error}"))?;
        fs::write(
            directory.join("localdeploy-runner.pid"),
            child.id().to_string(),
        )
        .map_err(|error| format!("Não foi possível registrar o processo do runner: {error}"))
    }
}

pub fn stop(directory: &Path) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = directory;
        Err("O runner self-hosted é disponibilizado pelo LocalDeploy somente no Windows.".into())
    }
    #[cfg(target_os = "windows")]
    {
        let pid = read_pid(directory)
            .ok_or("O LocalDeploy não encontrou um processo de runner iniciado por ele.")?;
        let status = hidden_command("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("Não foi possível encerrar o runner: {error}"))?;
        if !status.success() {
            return Err("O Windows não conseguiu encerrar o runner.".into());
        }
        let _ = fs::remove_file(directory.join("localdeploy-runner.pid"));
        Ok(())
    }
}

fn runner_directory(project: &Project) -> Result<PathBuf, String> {
    storage::app_data_dir()
        .map(|root| root.join("runners").join(project.id.to_string()))
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn github_client() -> Result<Client, String> {
    Client::builder()
        .build()
        .map_err(|error| format!("Não foi possível preparar a conexão com GitHub: {error}"))
}

#[cfg(target_os = "windows")]
fn github_request(client: &Client, token: &str, url: &str) -> reqwest::blocking::RequestBuilder {
    client
        .post(url)
        .header(USER_AGENT, "LocalDeploy")
        .header(ACCEPT, "application/vnd.github+json")
        .header(AUTHORIZATION, format!("Bearer {token}"))
}

#[cfg(target_os = "windows")]
fn request_registration_token(
    client: &Client,
    project: &Project,
    personal_token: &str,
) -> Result<String, String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/actions/runners/registration-token",
        project.repo_owner, project.repo_name
    );
    let response = github_request(client, personal_token, &url)
        .send()
        .map_err(|error| {
            format!("Não foi possível solicitar o token temporário do runner: {error}")
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub recusou o token para registrar o runner (HTTP {}). O token precisa ter permissão de administração de Actions neste repositório.",
            response.status()
        ));
    }
    let body: Value = response
        .json()
        .map_err(|error| format!("GitHub retornou uma resposta inválida: {error}"))?;
    body.get("token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or("GitHub não retornou um token temporário de registro.".into())
}

#[cfg(target_os = "windows")]
fn download_runner(client: &Client, directory: &Path) -> Result<(), String> {
    let release: Value = client
        .get("https://api.github.com/repos/actions/runner/releases/latest")
        .header(USER_AGENT, "LocalDeploy")
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .map_err(|error| format!("Não foi possível consultar a versão do runner oficial: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GitHub não disponibilizou o runner oficial: {error}"))?
        .json()
        .map_err(|error| format!("Resposta inválida ao consultar o runner: {error}"))?;
    let asset = release
        .get("assets")
        .and_then(Value::as_array)
        .and_then(|assets| {
            assets.iter().find(|asset| {
                asset
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| {
                        name.starts_with("actions-runner-win-x64-") && name.ends_with(".zip")
                    })
            })
        })
        .ok_or("A versão mais recente não possui runner Windows x64.")?;
    let url = asset
        .get("browser_download_url")
        .and_then(Value::as_str)
        .ok_or("O runner oficial não informou URL de download.")?;
    let mut response = client
        .get(url)
        .header(USER_AGENT, "LocalDeploy")
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("Não foi possível baixar o runner oficial: {error}"))?;
    let temporary = directory.with_extension("download");
    let _ = fs::remove_dir_all(&temporary);
    fs::create_dir_all(&temporary)
        .map_err(|error| format!("Não foi possível preparar a pasta do runner: {error}"))?;
    let archive_path = temporary.join("runner.zip");
    let mut file = File::create(&archive_path).map_err(|error| error.to_string())?;
    io::copy(&mut response, &mut file)
        .map_err(|error| format!("Não foi possível salvar o runner: {error}"))?;
    file.flush().map_err(|error| error.to_string())?;
    extract_zip(&archive_path, &temporary)?;
    fs::remove_file(&archive_path).map_err(|error| error.to_string())?;
    let _ = fs::remove_dir_all(directory);
    fs::rename(&temporary, directory)
        .map_err(|error| format!("Não foi possível finalizar o runner: {error}"))
}

#[cfg(target_os = "windows")]
fn extract_zip(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("O download do runner não é um ZIP válido: {error}"))?;
    for index in 0..archive.len() {
        let mut item = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some(relative) = item.enclosed_name() else {
            continue;
        };
        let output = destination.join(relative);
        if item.is_dir() {
            fs::create_dir_all(&output).map_err(|error| error.to_string())?;
        } else {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let mut output_file = File::create(&output).map_err(|error| error.to_string())?;
            io::copy(&mut item, &mut output_file).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn configure_runner(
    project: &Project,
    directory: &Path,
    registration_token: &str,
) -> Result<(), String> {
    let name = format!(
        "localdeploy-{}-{}",
        project.repo_name,
        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "windows".into())
    );
    let labels = format!("localdeploy,localdeploy-project-{}", project.id);
    let status = hidden_command("cmd")
        .args([
            "/C",
            "config.cmd",
            "--unattended",
            "--url",
            &project.repo_url,
            "--token",
            registration_token,
            "--name",
            &name,
            "--labels",
            &labels,
            "--work",
            "_work",
        ])
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("Não foi possível configurar o runner: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("O runner oficial não aceitou o registro. Verifique as permissões do token e tente novamente.".into())
    }
}

fn read_pid(directory: &Path) -> Option<u32> {
    fs::read_to_string(directory.join("localdeploy-runner.pid"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn process_is_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        hidden_command("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()
            .is_some_and(|output| {
                String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
            })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = pid;
        false
    }
}

#[cfg(test)]
mod workflow_tests {
    use super::*;

    #[test]
    fn adaptation_preserves_linux_and_backs_up_windows_workflow() {
        let temp = tempfile::tempdir().unwrap();
        let project = Project {
            id: 42,
            name: "fixture".into(),
            repo_url: "https://github.com/acme/fixture".into(),
            repo_owner: "acme".into(),
            repo_name: "fixture".into(),
            default_branch: "main".into(),
            local_path: temp.path().to_string_lossy().into_owned(),
            created_at: String::new(),
        };
        let root = temp.path().join(".github/workflows");
        fs::create_dir_all(&root).unwrap();
        let original = "on: push\njobs:\n  windows:\n    runs-on: windows-latest\n    steps: []\n  linux:\n    runs-on: ubuntu-latest\n    steps: []\n";
        fs::write(root.join("ci.yml"), original).unwrap();
        let report = adapt_workflows(&project).unwrap();
        assert!(report.contains("1 workflows"));
        assert_eq!(
            fs::read_to_string(root.join("ci.localdeploy-backup")).unwrap(),
            original
        );
        let converted: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(root.join("ci.yml")).unwrap()).unwrap();
        assert_eq!(
            converted["jobs"]["linux"]["runs-on"].as_str(),
            Some("ubuntu-latest")
        );
        assert!(converted["jobs"]["windows"]["runs-on"]
            .as_sequence()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("localdeploy-project-42")));
    }
}
