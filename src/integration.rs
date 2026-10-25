//! Stable, UI-independent facade for Kami Archs. Long operations return jobs.
use crate::{
    detector, environments, executor, gitops, preferences, process, runner, secrets,
    storage::{self, BuildRecord, NewProject, Project},
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

pub const PROTOCOL_VERSION: u32 = 1;
type EventHandler = Arc<dyn Fn(Value) + Send + Sync>;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: i32,
    pub message: String,
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} ({})", self.message, self.code)
    }
}
impl std::error::Error for ApiError {}
impl From<String> for ApiError {
    fn from(message: String) -> Self {
        Self {
            code: -32000,
            message,
        }
    }
}
impl From<storage::StorageError> for ApiError {
    fn from(error: storage::StorageError) -> Self {
        error.to_string().into()
    }
}
fn invalid(message: &str) -> ApiError {
    ApiError {
        code: -32602,
        message: message.into(),
    }
}
fn number(params: &Value, key: &str) -> Result<i64, ApiError> {
    params[key]
        .as_i64()
        .ok_or_else(|| invalid(&format!("Informe {key} como inteiro")))
}
fn string<'a>(params: &'a Value, key: &str) -> Result<&'a str, ApiError> {
    params[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid(&format!("Informe {key}")))
}
fn trusted(params: &Value) -> Result<(), ApiError> {
    if params["trusted"] == true {
        Ok(())
    } else {
        Err(invalid(
            "A IDE deve autorizar a execução com trusted=true após obter confiança no projeto",
        ))
    }
}

#[derive(Clone, Serialize)]
pub struct Job {
    pub id: u64,
    pub project_id: Option<i64>,
    pub kind: String,
    pub status: String,
    pub progress: String,
    pub result: Option<Value>,
    pub error: Option<String>,
}
struct Task {
    job: Job,
    cancel: Arc<AtomicBool>,
}
struct State {
    jobs: Mutex<HashMap<u64, Task>>,
    next: AtomicU64,
    events: Mutex<Option<EventHandler>>,
}
#[derive(Clone)]
pub struct Engine {
    state: Arc<State>,
}

impl Engine {
    /// One data directory per process. Use a dedicated engine subprocess for
    /// independent IDE instances that need different directories.
    pub fn open(data_directory: PathBuf) -> Result<Self, ApiError> {
        storage::configure_data_directory(data_directory)?;
        storage::list_projects()?;
        Ok(Self {
            state: Arc::new(State {
                jobs: Mutex::new(HashMap::new()),
                next: AtomicU64::new(1),
                events: Mutex::new(None),
            }),
        })
    }
    pub fn on_event(&self, callback: impl Fn(Value) + Send + Sync + 'static) {
        *self.state.events.lock().expect("events mutex") = Some(Arc::new(callback));
    }
    fn emit(&self, event: Value) {
        let callback = self.state.events.lock().expect("events mutex").clone();
        if let Some(callback) = callback {
            callback(event);
        }
    }
    pub fn job(&self, id: u64) -> Result<Job, ApiError> {
        self.state
            .jobs
            .lock()
            .expect("jobs mutex")
            .get(&id)
            .map(|task| task.job.clone())
            .ok_or_else(|| ApiError {
                code: -32004,
                message: "Tarefa não encontrada".into(),
            })
    }
    fn project(&self, params: &Value) -> Result<Project, ApiError> {
        let id = number(params, "project_id")?;
        storage::list_projects()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| ApiError {
                code: -32004,
                message: "Projeto não encontrado".into(),
            })
    }
    fn build(&self, params: &Value) -> Result<(Project, BuildRecord), ApiError> {
        let project = self.project(params)?;
        let id = number(params, "build_id")?;
        let build = storage::list_builds(project.id)?
            .into_iter()
            .find(|b| b.id == id)
            .ok_or_else(|| ApiError {
                code: -32004,
                message: "Build não encontrado neste projeto".into(),
            })?;
        Ok((project, build))
    }
    fn start_job(
        &self,
        project_id: Option<i64>,
        kind: &str,
        workspace: Option<PathBuf>,
        operation: impl FnOnce() -> Result<Value, String> + Send + 'static,
    ) -> Result<Value, ApiError> {
        let id = self.state.next.fetch_add(1, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        let job = Job {
            id,
            project_id,
            kind: kind.into(),
            status: "running".into(),
            progress: "Preparando…".into(),
            result: None,
            error: None,
        };
        {
            let mut jobs = self.state.jobs.lock().expect("jobs mutex");
            if jobs.values().any(|t| {
                t.job.project_id == project_id
                    && matches!(t.job.status.as_str(), "running" | "cancelling")
            }) {
                return Err(ApiError {
                    code: -32009,
                    message: "Já há uma tarefa em execução para este projeto".into(),
                });
            }
            jobs.insert(
                id,
                Task {
                    job: job.clone(),
                    cancel: cancel.clone(),
                },
            );
        }
        self.emit(json!({"type":"job.updated", "job":job}));
        let engine = self.clone();
        let finished = Arc::new(AtomicBool::new(false));
        if let Some(workspace) = workspace {
            let finished = finished.clone();
            let cancel = cancel.clone();
            let engine = self.clone();
            thread::spawn(move || {
                while !finished.load(Ordering::Acquire) {
                    if cancel.load(Ordering::Acquire) && workspace.is_dir() {
                        let _ = std::fs::write(workspace.join(".localdeploy-cancel"), "cancel");
                    }
                    if let Ok(progress) =
                        std::fs::read_to_string(workspace.join(".localdeploy-progress"))
                    {
                        let changed = {
                            let mut jobs = engine.state.jobs.lock().expect("jobs mutex");
                            let task = jobs.get_mut(&id).expect("job exists");
                            if task.job.progress != progress {
                                task.job.progress = progress;
                                Some(task.job.clone())
                            } else {
                                None
                            }
                        };
                        if let Some(job) = changed {
                            engine.emit(json!({"type":"job.updated", "job":job}));
                        }
                    }
                    thread::sleep(Duration::from_millis(300));
                }
            });
        }
        thread::spawn(move || {
            let outcome = if cancel.load(Ordering::Acquire) {
                Err("Tarefa cancelada".into())
            } else {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).unwrap_or_else(
                    |_| Err("O motor interrompeu a tarefa por uma falha interna".into()),
                )
            };
            finished.store(true, Ordering::Release);
            let job = {
                let mut jobs = engine.state.jobs.lock().expect("jobs mutex");
                let task = jobs.get_mut(&id).expect("job exists");
                task.job.status = if cancel.load(Ordering::Acquire) {
                    "cancelled"
                } else if outcome.is_ok() {
                    "succeeded"
                } else {
                    "failed"
                }
                .into();
                match outcome {
                    Ok(result) => task.job.result = Some(result),
                    Err(error) => task.job.error = Some(error),
                }
                task.job.clone()
            };
            engine.emit(json!({"type":"job.updated", "job":job}));
        });
        Ok(json!({"job_id":id}))
    }

    pub fn request(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        if !params.is_object() && !params.is_null() {
            return Err(invalid("params precisa ser um objeto"));
        }
        match method {
            "initialize" => Ok(
                json!({"name":"Kami Archs LocalDeploy", "version":env!("CARGO_PKG_VERSION"), "protocol_version":PROTOCOL_VERSION, "transport":"stdio-jsonrpc", "capabilities":["attach-local-repository","commit-history","async-build","preview-url","native-desktop","cancel","logs","cleanup","protected-variables"], "data_directory":storage::app_data_dir()?}),
            ),
            "projects.list" => Ok(json!(storage::list_projects()?)),
            "projects.attach" => {
                let root = PathBuf::from(string(&params, "path")?);
                if !root.is_absolute() || !root.is_dir() {
                    return Err(invalid(
                        "path precisa apontar para uma pasta absoluta existente",
                    ));
                }
                let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
                if !root.join(".git").exists() {
                    return Err(invalid(
                        "A pasta precisa ser a raiz de um repositório Git existente",
                    ));
                }
                if let Some(project) = storage::list_projects()?
                    .into_iter()
                    .find(|p| std::path::Path::new(&p.local_path) == root)
                {
                    return Ok(json!(project));
                }
                let branch = params["branch"].as_str().unwrap_or("main");
                if branch.is_empty() || branch.chars().any(char::is_control) {
                    return Err(invalid("Branch inválida"));
                }
                let new = if let Some(url) = params["repo_url"].as_str() {
                    NewProject::new(url, branch, root)?
                } else {
                    let name = root
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("workspace")
                        .to_owned();
                    NewProject {
                        repo_url: url::Url::from_directory_path(&root)
                            .map_err(|_| invalid("Pasta inválida"))?
                            .to_string(),
                        repo_owner: "local".into(),
                        repo_name: name,
                        default_branch: branch.into(),
                        local_path: root,
                    }
                };
                Ok(json!(storage::add_project(new)?))
            }
            "commits.list" => {
                let project = self.project(&params)?;
                Ok(json!(gitops::commits(
                    std::path::Path::new(&project.local_path),
                    params["limit"].as_u64().unwrap_or(50).clamp(1, 500) as usize,
                    &project.default_branch
                )?))
            }
            "projects.sync" => {
                let project = self.project(&params)?;
                self.start_job(Some(project.id), "sync", None, move || {
                    gitops::fetch(std::path::Path::new(&project.local_path))?;
                    Ok(json!({"synced":true}))
                })
            }
            "environment.inspect" => {
                let project = self.project(&params)?;
                let root = std::path::Path::new(&project.local_path);
                let backend = environments::detect(root);
                Ok(
                    json!({"stack":detector::analyze(root), "backend":backend.as_ref().ok().map(|b|format!("{b:?}")), "blocker":backend.err(), "requires_docker":!matches!(environments::detect(root), Ok(environments::Backend::DioxusDesktop)), "runtime_validation":"deferred_until_build"}),
                )
            }
            "builds.plan" => {
                let project = self.project(&params)?;
                Ok(json!(executor::plan(&project, string(&params, "commit")?)?))
            }
            "builds.list" => Ok(json!(storage::list_builds(self.project(&params)?.id)?)),
            "builds.prepare" => {
                trusted(&params)?;
                let project = self.project(&params)?;
                let sha = gitops::resolve_commit(
                    std::path::Path::new(&project.local_path),
                    string(&params, "commit")?,
                )?;
                if storage::list_builds(project.id)?.iter().any(|build| {
                    build.commit_sha == sha && build.status == "ready" && build.process_id.is_some()
                }) {
                    return Err(ApiError { code: -32009, message: "Este commit já tem um preview ativo. Abra o build existente ou pare/remova o preview antes de reconstruir.".into() });
                }
                let plan = executor::plan(&project, &sha)?;
                let run_tests = params["run_tests"].as_bool().unwrap_or(true);
                self.start_job(
                    Some(project.id),
                    "prepare",
                    Some(plan.workspace),
                    move || Ok(json!(executor::prepare_preview(&project, &sha, run_tests)?)),
                )
            }
            "previews.open" => {
                let (project, build) = self.build(&params)?;
                self.start_job(Some(project.id), "open", None, move || {
                    Ok(json!(executor::start_preview(&project, &build)?))
                })
            }
            "previews.stop" => {
                let (project, build) = self.build(&params)?;
                self.start_job(Some(project.id), "stop", None, move || {
                    executor::stop_preview(&build)?;
                    Ok(json!({"stopped":true}))
                })
            }
            "previews.remove" => {
                let (project, build) = self.build(&params)?;
                self.start_job(Some(project.id), "remove", None, move || {
                    executor::delete_preview_workspace(&project, &build)?;
                    Ok(json!({"removed":true}))
                })
            }
            "logs.read" => {
                let (_, build) = self.build(&params)?;
                Ok(json!({"text":executor::read_log(&build)?}))
            }
            "jobs.get" => Ok(json!(self.job(number(&params, "job_id")? as u64)?)),
            "jobs.cancel" => {
                let id = number(&params, "job_id")? as u64;
                let job = {
                    let mut jobs = self.state.jobs.lock().expect("jobs mutex");
                    let task = jobs
                        .get_mut(&id)
                        .ok_or_else(|| invalid("Tarefa não encontrada"))?;
                    if task.job.kind != "prepare" {
                        return Err(invalid(
                            "Somente tarefas de preparação aceitam cancelamento",
                        ));
                    }
                    if task.job.status == "running" {
                        task.cancel.store(true, Ordering::Release);
                        task.job.status = "cancelling".into();
                    }
                    task.job.clone()
                };
                self.emit(json!({"type":"job.updated","job":job}));
                Ok(json!(job))
            }
            "settings.read" => Ok(json!(preferences::load())),
            "settings.write" => {
                let settings =
                    serde_json::from_value(params).map_err(|e| invalid(&e.to_string()))?;
                preferences::save(&settings)?;
                Ok(json!(settings))
            }
            "variables.write" => {
                let project = self.project(&params)?;
                secrets::save(
                    project.id,
                    params["values"]
                        .as_str()
                        .ok_or_else(|| invalid("Informe values como texto NOME=valor"))?,
                )?;
                Ok(json!({"saved":true}))
            }
            "environment.prepare" => {
                trusted(&params)?;
                self.start_job(None, "tools", None, move || {
                    process::install_prerequisites()?;
                    Ok(json!({"prepared":true,"restart_engine":true}))
                })
            }
            "runner.workflow.generate" => {
                trusted(&params)?;
                let project = self.project(&params)?;
                runner::generate_workflow(&project)?;
                Ok(json!({"created":true,"requires_commit_and_push":true}))
            }
            "shutdown" => {
                let jobs = self.state.jobs.lock().expect("jobs mutex");
                if jobs
                    .values()
                    .any(|task| matches!(task.job.status.as_str(), "running" | "cancelling"))
                {
                    return Err(ApiError {
                        code: -32009,
                        message: "Conclua ou cancele as tarefas antes de encerrar o motor".into(),
                    });
                }
                Ok(json!({"shutdown":true,"previews_continue":true}))
            }
            _ => Err(ApiError {
                code: -32601,
                message: format!("Método desconhecido: {method}"),
            }),
        }
    }
}
