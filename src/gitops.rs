use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Stdio;

use crate::process::hidden_command;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Commit {
    pub sha: String,
    pub short_sha: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    pub location: CommitLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitLocation {
    Local,
    Branch,
    Main,
}

impl CommitLocation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Branch => "Branch",
            Self::Main => "Main",
        }
    }
}

fn git(args: &[&str]) -> Result<String, String> {
    let output = hidden_command("git")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Git não está instalado ou não foi encontrado no PATH: {e}"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if message.is_empty() {
            "O Git encerrou a operação com erro.".into()
        } else {
            message
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn clone_repository(url: &str, branch: &str, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        let empty = destination.is_dir()
            && std::fs::read_dir(destination)
                .map_err(|e| e.to_string())?
                .next()
                .is_none();
        if !empty {
            return Err("A pasta escolhida já existe e não está vazia.".into());
        }
        std::fs::remove_dir(destination)
            .map_err(|e| format!("Não foi possível preparar a pasta: {e}"))?;
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Não foi possível criar a pasta pai: {e}"))?;
    }
    let destination = destination
        .to_str()
        .ok_or("O caminho local não é válido para o Git.")?;
    match git(&["clone", "--branch", branch, "--", url, destination]) {
        Ok(_) => Ok(()),
        Err(error) => {
            // Clone can leave a partial checkout behind; it belongs to this failed operation.
            let _ = std::fs::remove_dir_all(destination);
            Err(error)
        }
    }
}

/// Opens Git Credential Manager's browser sign-in when it is supplied by Git
/// for Windows. Public repositories can be cloned without this step.
pub fn open_github_sign_in() -> Result<(), String> {
    for helper in ["credential-manager", "credential-manager-core"] {
        let available = hidden_command("git")
            .args([helper, "--version"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if available {
            return hidden_command("git")
                .args([helper, "github", "login"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map(|_| ())
                .map_err(|error| format!("Não foi possível abrir o login do GitHub: {error}"));
        }
    }
    Err("O Git Credential Manager não foi encontrado. Instale ou atualize o Git for Windows e tente novamente.".into())
}

pub fn commits(path: &Path, limit: usize, default_branch: &str) -> Result<Vec<Commit>, String> {
    let path = path
        .to_str()
        .ok_or("O caminho local não é válido para o Git.")?;
    let history_limit = limit.max(2_000);
    let output = git(&[
        "-C",
        path,
        "log",
        "--all",
        "--date=short",
        "--decorate=full",
        "--format=%H%x1f%h%x1f%an%x1f%ad%x1f%s%x1f%P%x1f%D",
        "-n",
        &history_limit.to_string(),
    ])?;
    struct ParsedCommit {
        commit: Commit,
        parents: Vec<String>,
        decorations: String,
    }
    let parsed = output
        .lines()
        .filter_map(|line| {
            let fields = line.splitn(7, '\x1f').collect::<Vec<_>>();
            (fields.len() == 7).then(|| ParsedCommit {
                commit: Commit {
                    sha: fields[0].into(),
                    short_sha: fields[1].into(),
                    author: fields[2].into(),
                    date: fields[3].into(),
                    subject: fields[4].into(),
                    location: CommitLocation::Local,
                },
                parents: fields[5].split_whitespace().map(str::to_owned).collect(),
                decorations: fields[6].into(),
            })
        })
        .collect::<Vec<_>>();

    let parents = parsed
        .iter()
        .map(|item| (item.commit.sha.clone(), item.parents.clone()))
        .collect::<HashMap<_, _>>();
    let main_ref = format!("refs/remotes/origin/{default_branch}");
    let local_main_ref = format!("refs/heads/{default_branch}");
    let mut main_tips = Vec::new();
    let mut remote_tips = Vec::new();
    for item in &parsed {
        for decoration in item.decorations.split(',').map(str::trim) {
            let decoration = decoration.strip_prefix("HEAD -> ").unwrap_or(decoration);
            if decoration == main_ref || decoration == local_main_ref {
                main_tips.push(item.commit.sha.clone());
            } else if decoration.starts_with("refs/remotes/origin/")
                && !decoration.ends_with("/HEAD")
            {
                remote_tips.push(item.commit.sha.clone());
            }
        }
    }
    let main_commits = reachable_commits(&main_tips, &parents);
    let remote_commits = reachable_commits(&remote_tips, &parents);
    Ok(parsed
        .into_iter()
        .take(limit)
        .map(|mut item| {
            item.commit.location = if main_commits.contains(&item.commit.sha) {
                CommitLocation::Main
            } else if remote_commits.contains(&item.commit.sha) {
                CommitLocation::Branch
            } else {
                CommitLocation::Local
            };
            item.commit
        })
        .collect())
}

fn reachable_commits(tips: &[String], parents: &HashMap<String, Vec<String>>) -> HashSet<String> {
    let mut reachable = HashSet::new();
    let mut pending = tips.to_vec();
    while let Some(sha) = pending.pop() {
        if !reachable.insert(sha.clone()) {
            continue;
        }
        if let Some(commit_parents) = parents.get(&sha) {
            pending.extend(commit_parents.iter().cloned());
        }
    }
    reachable
}

pub fn fetch(path: &Path) -> Result<(), String> {
    let path = path
        .to_str()
        .ok_or("O caminho local não é válido para o Git.")?;
    git(&["-C", path, "fetch", "--prune", "origin"]).map(|_| ())
}

pub fn resolve_commit(path: &Path, reference: &str) -> Result<String, String> {
    if reference.is_empty()
        || reference.len() > 80
        || reference.chars().any(|character| character.is_control())
    {
        return Err("A referência do commit não é válida.".into());
    }
    let path = path
        .to_str()
        .ok_or("O caminho local não é válido para o Git.")?;
    let sha = git(&[
        "-C",
        path,
        "rev-parse",
        "--verify",
        "--end-of-options",
        &format!("{reference}^{{commit}}"),
    ])?;
    if sha.len() == 40 && sha.chars().all(|character| character.is_ascii_hexdigit()) {
        Ok(sha)
    } else {
        Err("O Git não retornou um SHA de commit válido.".into())
    }
}

pub fn create_worktree(repository: &Path, sha: &str, destination: &Path) -> Result<(), String> {
    if sha.len() != 40 || !sha.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err("O SHA do commit não é válido.".into());
    }
    if destination.exists() {
        return if destination.join(".git").exists() {
            Ok(())
        } else {
            Err("O diretório de preview já existe e não pertence a um worktree.".into())
        };
    }
    let repository = repository
        .to_str()
        .ok_or("O caminho local não é válido para o Git.")?;
    let destination = destination
        .to_str()
        .ok_or("O destino do workspace não é válido para o Git.")?;
    let _ = git(&["-C", repository, "worktree", "prune"]);
    git(&[
        "-C",
        repository,
        "-c",
        "core.hooksPath=",
        "worktree",
        "add",
        "--detach",
        "--",
        destination,
        sha,
    ])
    .map(|_| ())
}

pub fn remove_worktree(repository: &Path, destination: &Path) -> Result<(), String> {
    let repository = repository
        .to_str()
        .ok_or("O caminho local não é válido para o Git.")?;
    let destination_text = destination
        .to_str()
        .ok_or("O destino do workspace não é válido para o Git.")?;
    if destination.exists() {
        git(&[
            "-C",
            repository,
            "worktree",
            "remove",
            "--force",
            "--",
            destination_text,
        ])?;
    }
    let _ = git(&["-C", repository, "worktree", "prune"]);
    Ok(())
}
