use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use std::sync::OnceLock;
use thiserror::Error;
use url::Url;

static DATA_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

/// Configure once, before starting worker threads. Different roots require
/// separate engine processes; global environment variables are not mutated.
pub fn configure_data_directory(directory: PathBuf) -> Result<(), StorageError> {
    if !directory.is_absolute() {
        return Err(StorageError::RelativePath);
    }
    std::fs::create_dir_all(&directory)
        .map_err(|e| StorageError::StorageDirectory(e.to_string()))?;
    let directory = std::fs::canonicalize(directory)
        .map_err(|e| StorageError::StorageDirectory(e.to_string()))?;
    if let Some(existing) = DATA_DIRECTORY.get() {
        return if existing == &directory {
            Ok(())
        } else {
            Err(StorageError::StorageDirectory(
                "Já há um motor usando outra pasta de dados neste processo".into(),
            ))
        };
    }
    DATA_DIRECTORY.set(directory).map_err(|_| {
        StorageError::StorageDirectory("Pasta de dados configurada simultaneamente".into())
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub repo_url: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub default_branch: String,
    pub local_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProject {
    pub repo_url: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub default_branch: String,
    pub local_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BuildRecord {
    pub id: i64,
    pub project_id: i64,
    pub commit_sha: String,
    pub environment: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub log_path: String,
    pub workspace_path: String,
    pub endpoint: Option<String>,
    pub process_id: Option<i64>,
    pub error_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewBuild<'a> {
    pub project_id: i64,
    pub commit_sha: &'a str,
    pub environment: &'a str,
    pub log_path: &'a Path,
    pub workspace_path: &'a Path,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("URL inválida. Use https://github.com/organizacao/repositorio.")]
    InvalidGithubUrl,
    #[error("Informe uma branch padrão válida.")]
    InvalidBranch,
    #[error("O endereço local precisa ser absoluto.")]
    RelativePath,
    #[error("Este repositório já está cadastrado.")]
    DuplicateRepository,
    #[error("O armazenamento local não está disponível: {0}")]
    StorageDirectory(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

impl NewProject {
    pub fn new(repo_url: &str, branch: &str, local_path: PathBuf) -> Result<Self, StorageError> {
        let (repo_owner, repo_name) = parse_github_url(repo_url)?;
        let branch = branch.trim();
        if branch.is_empty() || branch.chars().any(char::is_control) {
            return Err(StorageError::InvalidBranch);
        }
        if !local_path.is_absolute() {
            return Err(StorageError::RelativePath);
        }
        Ok(Self {
            repo_url: format!("https://github.com/{repo_owner}/{repo_name}"),
            repo_owner,
            repo_name,
            default_branch: branch.to_string(),
            local_path,
        })
    }
}

pub fn repo_name_from_url(value: &str) -> Result<String, StorageError> {
    parse_github_url(value).map(|(_, name)| name)
}

fn parse_github_url(value: &str) -> Result<(String, String), StorageError> {
    let parsed = Url::parse(value.trim()).map_err(|_| StorageError::InvalidGithubUrl)?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(StorageError::InvalidGithubUrl);
    }

    let segments = parsed
        .path_segments()
        .ok_or(StorageError::InvalidGithubUrl)?
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if segments.len() != 2 {
        return Err(StorageError::InvalidGithubUrl);
    }

    let owner = segments[0].to_ascii_lowercase();
    let repo = segments[1]
        .strip_suffix(".git")
        .unwrap_or(segments[1])
        .to_ascii_lowercase();
    if owner.is_empty()
        || repo.is_empty()
        || owner.chars().any(char::is_control)
        || repo.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidGithubUrl);
    }
    Ok((owner, repo))
}

fn db_path() -> Result<PathBuf, StorageError> {
    let root = app_data_dir()?;
    Ok(root.join("localdeploy.sqlite3"))
}

pub fn app_data_dir() -> Result<PathBuf, StorageError> {
    let root = if let Some(directory) = DATA_DIRECTORY.get() {
        directory.clone()
    } else if let Some(directory) = std::env::var_os("LOCALDEPLOY_DATA_DIR") {
        let directory = PathBuf::from(directory);
        if !directory.is_absolute() {
            return Err(StorageError::StorageDirectory(
                "LOCALDEPLOY_DATA_DIR precisa ser um caminho absoluto".into(),
            ));
        }
        directory
    } else if let Some(package) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .filter(|root| root.join("portable.flag").is_file())
    {
        package.join("Data")
    } else {
        dirs::data_local_dir()
            .ok_or_else(|| {
                StorageError::StorageDirectory("pasta de dados do usuário não encontrada".into())
            })?
            .join("LocalDeploy")
    };
    std::fs::create_dir_all(&root)
        .map_err(|error| StorageError::StorageDirectory(error.to_string()))?;
    Ok(root)
}

fn open_at(path: &Path) -> Result<Connection, StorageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| StorageError::StorageDirectory(error.to_string()))?;
    }
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            repo_url TEXT NOT NULL UNIQUE,
            repo_owner TEXT NOT NULL,
            repo_name TEXT NOT NULL,
            default_branch TEXT NOT NULL,
            local_path TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );
        CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name COLLATE NOCASE);
        CREATE TABLE IF NOT EXISTS builds (
            id INTEGER PRIMARY KEY,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            commit_sha TEXT NOT NULL,
            environment TEXT NOT NULL CHECK(environment IN ('preview', 'production')),
            status TEXT NOT NULL,
            started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
            finished_at TEXT,
            log_path TEXT NOT NULL,
            workspace_path TEXT NOT NULL,
            endpoint TEXT,
            process_id INTEGER,
            error_summary TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_builds_project_started ON builds(project_id, id DESC);
        CREATE TABLE IF NOT EXISTS deployments (
            id INTEGER PRIMARY KEY,
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE RESTRICT,
            target TEXT NOT NULL CHECK(target = 'production'),
            status TEXT NOT NULL,
            endpoint TEXT NOT NULL,
            deployed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
            superseded_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_deployments_project_active ON deployments(project_id, target, superseded_at);
        PRAGMA user_version = 2;",
    )?;
    Ok(connection)
}

fn open_default() -> Result<Connection, StorageError> {
    let path = db_path()?;
    let connection = open_at(&path)?;
    let root = path
        .parent()
        .ok_or_else(|| StorageError::StorageDirectory("Pasta de dados inválida".into()))?;
    let current = root.to_string_lossy().into_owned();
    let marker = root.join("library-location.txt");
    if let Ok(previous) = std::fs::read_to_string(&marker) {
        if previous == current {
            return Ok(connection);
        }
        if previous != current && !previous.is_empty() {
            for (table, column) in [
                ("projects", "local_path"),
                ("builds", "workspace_path"),
                ("builds", "log_path"),
            ] {
                connection.execute(&format!("UPDATE {table} SET {column} = ?1 || substr({column}, length(?2) + 1) WHERE substr({column}, 1, length(?2) + 1) IN (?2 || '/', ?2 || char(92))"), params![current, previous])?;
            }
        }
    }
    std::fs::write(marker, current).map_err(|e| StorageError::StorageDirectory(e.to_string()))?;
    Ok(connection)
}

pub fn list_projects() -> Result<Vec<Project>, StorageError> {
    let connection = open_default()?;
    let mut statement = connection.prepare(
        "SELECT id, name, repo_url, repo_owner, repo_name, default_branch, local_path, created_at
         FROM projects ORDER BY name COLLATE NOCASE, id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            repo_url: row.get(2)?,
            repo_owner: row.get(3)?,
            repo_name: row.get(4)?,
            default_branch: row.get(5)?,
            local_path: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub fn add_project(project: NewProject) -> Result<Project, StorageError> {
    let connection = open_default()?;
    let name = project.repo_name.clone();
    let path = project.local_path.to_string_lossy().into_owned();
    let insert = connection.execute(
        "INSERT INTO projects (name, repo_url, repo_owner, repo_name, default_branch, local_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            name,
            project.repo_url,
            project.repo_owner,
            project.repo_name,
            project.default_branch,
            path
        ],
    );
    match insert {
        Ok(_) => {}
        Err(rusqlite::Error::SqliteFailure(code, _))
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            return Err(StorageError::DuplicateRepository);
        }
        Err(error) => return Err(StorageError::Sqlite(error)),
    }
    let id = connection.last_insert_rowid();
    connection
        .query_row(
            "SELECT id, name, repo_url, repo_owner, repo_name, default_branch, local_path, created_at
             FROM projects WHERE id = ?1",
            [id],
            |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    repo_url: row.get(2)?,
                    repo_owner: row.get(3)?,
                    repo_name: row.get(4)?,
                    default_branch: row.get(5)?,
                    local_path: row.get(6)?,
                    created_at: row.get(7)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| StorageError::StorageDirectory("registro recém-criado não foi encontrado".into()))
}

pub fn delete_project(id: i64) -> Result<(), StorageError> {
    let connection = open_default()?;
    connection.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(())
}

pub fn create_build(build: NewBuild<'_>) -> Result<BuildRecord, StorageError> {
    let connection = open_default()?;
    connection.execute(
        "INSERT INTO builds (project_id, commit_sha, environment, status, log_path, workspace_path)
         VALUES (?1, ?2, ?3, 'analyzing', ?4, ?5)",
        params![
            build.project_id,
            build.commit_sha,
            build.environment,
            build.log_path.to_string_lossy(),
            build.workspace_path.to_string_lossy(),
        ],
    )?;
    get_build_at(&connection, connection.last_insert_rowid())
}

pub fn update_build(
    id: i64,
    status: &str,
    endpoint: Option<&str>,
    process_id: Option<u32>,
    error_summary: Option<&str>,
    finished: bool,
) -> Result<(), StorageError> {
    let connection = open_default()?;
    connection.execute(
        "UPDATE builds SET status = ?2, endpoint = ?3, process_id = ?4, error_summary = ?5,
            finished_at = CASE WHEN ?6 THEN strftime('%Y-%m-%dT%H:%M:%SZ', 'now') ELSE finished_at END
         WHERE id = ?1",
        params![id, status, endpoint, process_id.map(i64::from), error_summary, finished],
    )?;
    Ok(())
}

pub fn list_builds(project_id: i64) -> Result<Vec<BuildRecord>, StorageError> {
    let connection = open_default()?;
    let mut statement = connection.prepare(
        "SELECT id, project_id, commit_sha, environment, status, started_at, finished_at, log_path,
                workspace_path, endpoint, process_id, error_summary
         FROM builds WHERE project_id = ?1 ORDER BY id DESC LIMIT 100",
    )?;
    let rows = statement.query_map([project_id], build_from_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub fn promote_build(project_id: i64, build_id: i64, endpoint: &str) -> Result<(), StorageError> {
    let mut connection = open_default()?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE deployments SET superseded_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE project_id = ?1 AND target = 'production' AND superseded_at IS NULL",
        [project_id],
    )?;
    transaction.execute(
        "INSERT INTO deployments (project_id, build_id, target, status, endpoint)
         VALUES (?1, ?2, 'production', 'ready', ?3)",
        params![project_id, build_id, endpoint],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn active_production(project_id: i64) -> Result<Option<BuildRecord>, StorageError> {
    let connection = open_default()?;
    connection
        .query_row(
            "SELECT b.id, b.project_id, b.commit_sha, b.environment, b.status, b.started_at,
                    b.finished_at, b.log_path, b.workspace_path, b.endpoint, b.process_id, b.error_summary
             FROM deployments d
             JOIN builds b ON b.id = d.build_id
             WHERE d.project_id = ?1 AND d.target = 'production' AND d.superseded_at IS NULL
             ORDER BY d.id DESC LIMIT 1",
            [project_id],
            build_from_row,
        )
        .optional()
        .map_err(StorageError::from)
}

fn get_build_at(connection: &Connection, id: i64) -> Result<BuildRecord, StorageError> {
    connection.query_row(
        "SELECT id, project_id, commit_sha, environment, status, started_at, finished_at, log_path,
                workspace_path, endpoint, process_id, error_summary
         FROM builds WHERE id = ?1",
        [id],
        build_from_row,
    ).map_err(StorageError::from)
}

fn build_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BuildRecord> {
    Ok(BuildRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        commit_sha: row.get(2)?,
        environment: row.get(3)?,
        status: row.get(4)?,
        started_at: row.get(5)?,
        finished_at: row.get(6)?,
        log_path: row.get(7)?,
        workspace_path: row.get(8)?,
        endpoint: row.get(9)?,
        process_id: row.get(10)?,
        error_summary: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_github_urls_and_git_suffix() {
        assert_eq!(
            parse_github_url("https://github.com/acme/site").unwrap(),
            ("acme".into(), "site".into())
        );
        assert_eq!(
            parse_github_url("https://github.com/acme/site.git").unwrap(),
            ("acme".into(), "site".into())
        );
    }

    #[test]
    fn rejects_urls_that_could_change_the_clone_target() {
        for value in [
            "http://github.com/acme/site",
            "https://github.com.evil.test/acme/site",
            "https://user@github.com/acme/site",
            "https://github.com/acme/site/tree/main",
            "https://github.com/acme/site?redirect=1",
        ] {
            assert!(parse_github_url(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn project_records_persist_and_delete_in_sqlite() {
        let directory = tempfile::tempdir().unwrap();
        let db = directory.path().join("state.sqlite3");
        let connection = open_at(&db).unwrap();
        connection.execute(
            "INSERT INTO projects (name, repo_url, repo_owner, repo_name, default_branch, local_path)
             VALUES ('site', 'https://github.com/acme/site', 'acme', 'site', 'main', '/tmp/site')",
            [],
        ).unwrap();
        drop(connection);

        let reopened = open_at(&db).unwrap();
        let count: i64 = reopened
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        reopened
            .execute(
                "DELETE FROM projects WHERE repo_url = ?1",
                ["https://github.com/acme/site"],
            )
            .unwrap();
        let count: i64 = reopened
            .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn build_records_are_persisted_with_the_project() {
        let directory = tempfile::tempdir().unwrap();
        let db = directory.path().join("state.sqlite3");
        let connection = open_at(&db).unwrap();
        connection
            .execute(
                "INSERT INTO projects (name, repo_url, repo_owner, repo_name, default_branch, local_path)
                 VALUES ('site', 'https://github.com/acme/site', 'acme', 'site', 'main', '/tmp/site')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO builds (project_id, commit_sha, environment, status, log_path, workspace_path)
                 VALUES (1, '0123456789012345678901234567890123456789', 'preview', 'ready', '/tmp/log', '/tmp/workspace')",
                [],
            )
            .unwrap();
        let build: BuildRecord = connection
            .query_row(
                "SELECT id, project_id, commit_sha, environment, status, started_at, finished_at, log_path,
                        workspace_path, endpoint, process_id, error_summary FROM builds WHERE id = 1",
                [],
                build_from_row,
            )
            .unwrap();
        assert_eq!(build.status, "ready");
        assert_eq!(build.project_id, 1);
    }
}
