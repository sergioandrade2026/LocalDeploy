#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use localdeploy::{
    detector, executor, gitops, preferences, process, runner, secrets, server, storage,
};
mod ux;

use dioxus::prelude::*;
use storage::{NewProject, Project};
use ux::{ConnectionsDialog, PreferencesPanel};

const STYLE: &str = include_str!("../assets/main.css");

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--build-commit") {
        let result = (|| -> Result<(), String> {
            let id = std::env::args()
                .nth(2)
                .ok_or("Projeto ausente")?
                .parse::<i64>()
                .map_err(|e| e.to_string())?;
            let sha = std::env::args().nth(3).ok_or("SHA ausente")?;
            let project = storage::list_projects()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|p| p.id == id)
                .ok_or("Projeto não encontrado")?;
            gitops::fetch(std::path::Path::new(&project.local_path))?;
            executor::prepare_preview(&project, &sha, true)?;
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(result) = server::run_from_args() {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    configure_webview_runtime();
    dioxus::LaunchBuilder::new()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_window(dioxus::desktop::WindowBuilder::new().with_title("LocalDeploy"))
                .with_menu(None),
        )
        .launch(App);
}

#[cfg(target_os = "windows")]
fn configure_webview_runtime() {
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(package_dir) = executable.parent() else {
        return;
    };
    let runtime_root = package_dir.join("WebView2Runtime");
    let Ok(entries) = std::fs::read_dir(runtime_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.join("msedgewebview2.exe").is_file() {
            std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", path);
            break;
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn configure_webview_runtime() {}

#[component]
fn App() -> Element {
    let mut projects = use_signal(Vec::<Project>::new);
    let mut search = use_signal(String::new);
    let mut show_new_project = use_signal(|| false);
    let mut notice = use_signal(|| None::<String>);
    let mut selected_nav = use_signal(|| "Projetos".to_string());
    let mut selected_project = use_signal(|| None::<Project>);
    let mut preferences = use_signal(preferences::load);
    let mut show_connections = use_signal(|| false);

    use_future(move || async move {
        match storage::list_projects() {
            Ok(items) => projects.set(items),
            Err(error) => notice.set(Some(format!(
                "Não foi possível abrir a biblioteca: {error}"
            ))),
        }
    });
    use_future(move || async move {
        let _ = tokio::task::spawn_blocking(executor::cleanup_ephemeral_workspaces).await;
    });

    let filtered = projects()
        .into_iter()
        .filter(|project| {
            let query = search().to_lowercase();
            query.is_empty()
                || project.name.to_lowercase().contains(&query)
                || project.repo_url.to_lowercase().contains(&query)
        })
        .collect::<Vec<_>>();

    rsx! {
        style { "{STYLE}" }
        style { "body {{ zoom: {preferences().font_size as f32 / 16.0}; }}" }
        if preferences().theme == "Escuro" {
            style { "html {{ filter: invert(.9) hue-rotate(180deg); }}" }
        }
        div { class: "app-shell",
            aside { class: "sidebar",
                div { class: "brand",
                    div { class: "brand-mark", "L" }
                    div { class: "brand-copy",
                        strong { "localdeploy" }
                        span { "DESKTOP" }
                    }
                }
                div { class: "workspace-label", "WORKSPACE LOCAL" }
                nav { class: "nav-list",
                    if selected_project().is_some() {
                        button { class: if selected_nav() == "Deployments" { "nav-item active" } else { "nav-item" }, onclick: move |_| selected_nav.set("Deployments".into()), span { class: "nav-icon", "◫" } "Deployments" }
                        button { class: "nav-item", onclick: move |_| selected_nav.set("Logs".into()), span { class: "nav-icon", "☷" } "Logs" }
                        button { class: "nav-item", onclick: move |_| selected_nav.set("Configurações".into()), span { class: "nav-icon", "⚙" } "Configurações" }
                    } else {
                        button {
                            class: if selected_nav() == "Projetos" { "nav-item active" } else { "nav-item" },
                            onclick: move |_| { selected_nav.set("Projetos".to_string()); selected_project.set(None); },
                            span { class: "nav-icon", "▦" } "Projetos"
                            span { class: "nav-count", "{projects().len()}" }
                        }
                    }
                }
                div { class: "sidebar-bottom",
                    button { class: "nav-item", onclick: move |_| show_connections.set(true), "Conexões" }
                    button { class: "nav-item", onclick: move |_| { selected_project.set(None); selected_nav.set("Preferências".into()); }, "Preferências" }
                    div { class: "local-status",
                        span { class: "status-dot" }
                        div { "Executando neste dispositivo" }
                    }
                    div { class: "sidebar-footer", "LocalDeploy · versão 0.6.0" }
                }
            }

            main { class: "main-area",
                header { class: "topbar",
                    div { class: "breadcrumb", "Workspace", span { "/" }, strong { "{selected_nav()}" } }
                    div { class: "topbar-right",
                        span { class: "local-pill", span { class: "status-dot" } "Somente local" }
                        button { class: "avatar", "S" }
                    }
                }

                if let Some(project) = selected_project() {
                    ProjectDetail {
                        project,
                        tab: selected_nav(),
                        on_back: move |_| { selected_project.set(None); selected_nav.set("Projetos".into()); },
                    }
                } else if selected_nav() == "Preferências" {
                    PreferencesPanel { value: preferences(), on_saved: move |value| preferences.set(value) }
                } else if selected_nav() == "Projetos" {
                    section { class: "content",
                        div { class: "page-heading",
                            div {
                                div { class: "eyebrow", "PAINEL DE DESENVOLVIMENTO" }
                                h1 { "Seus projetos" }
                                p { "Gerencie projetos locais e acompanhe cada etapa do desenvolvimento." }
                            }
                            button { class: "primary-button", onclick: move |_| show_new_project.set(true), span { "＋" } " Novo projeto" }
                        }

                        if let Some(message) = notice() {
                            div { class: "notice", "{message}", button { onclick: move |_| notice.set(None), "Fechar" } }
                        }

                        div { class: "stats-row",
                            div { class: "stat-card",
                                div { class: "stat-label", "PROJETOS NA BIBLIOTECA" }
                                div { class: "stat-value", "{projects().len()}" }
                                div { class: "stat-caption", "Salvos neste dispositivo" }
                            }
                            div { class: "stat-card muted-stat",
                                div { class: "stat-label", "DEPLOYS ATIVOS" }
                                div { class: "stat-value", "—" }
                                div { class: "stat-caption", "Disponível nas próximas etapas" }
                            }
                            div { class: "stat-card muted-stat",
                                div { class: "stat-label", "ÚLTIMA ATIVIDADE" }
                                div { class: "stat-value stat-placeholder", "Ainda sem atividade" }
                                div { class: "stat-caption", "Os eventos aparecerão aqui" }
                            }
                        }

                        div { class: "section-toolbar",
                            div { class: "section-title",
                                h2 { "Biblioteca" }
                                span { class: "count-badge", "{filtered.len()}" }
                            }
                            label { class: "search-box",
                                span { "⌕" }
                                input {
                                    placeholder: "Buscar projeto...",
                                    value: "{search}",
                                    oninput: move |event| search.set(event.value()),
                                }
                                kbd { "⌘ K" }
                            }
                        }

                        if filtered.is_empty() {
                            div { class: "empty-state",
                                div { class: "empty-art",
                                    div { class: "empty-window",
                                        div { class: "window-top" }
                                        div { class: "window-line wide" }
                                        div { class: "window-line" }
                                        div { class: "window-line short" }
                                    }
                                    div { class: "empty-plus", "+" }
                                }
                                h3 { if projects().is_empty() { "Sua biblioteca começa aqui" } else { "Nenhum projeto encontrado" } }
                                p { if projects().is_empty() { "Adicione um projeto para começar a organizar seus ambientes de desenvolvimento." } else { "Tente outro nome ou URL de repositório." } }
                                if projects().is_empty() {
                                    button { class: "secondary-button", onclick: move |_| show_new_project.set(true), "Adicionar primeiro projeto" }
                                }
                            }
                        } else {
                            div { class: "project-grid",
                                for project in filtered {
                                    ProjectCard {
                                        key: "{project.id}",
                                        project: project.clone(),
                                        is_cloned: std::path::Path::new(&project.local_path).join(".git").exists(),
                                        on_open: move |project: Project| { selected_project.set(Some(project)); selected_nav.set("Deployments".into()); },
                                        on_clone: move |project: Project| {
                                            notice.set(Some(format!("Clonando {} em segundo plano…", project.name)));
                                            spawn(async move {
                                                let cloned_name = project.name.clone();
                                                let result = tokio::task::spawn_blocking(move || gitops::clone_repository(
                                                    &project.repo_url,
                                                    &project.default_branch,
                                                    std::path::Path::new(&project.local_path),
                                                )).await;
                                                match result {
                                                    Ok(Ok(())) => {
                                                        if let Ok(items) = storage::list_projects() { projects.set(items); }
                                                        notice.set(Some(format!("{cloned_name} foi clonado com sucesso.")));
                                                    }
                                                    Ok(Err(error)) => notice.set(Some(format!("Não foi possível clonar o repositório: {error}"))),
                                                    Err(error) => notice.set(Some(format!("A clonagem foi interrompida: {error}"))),
                                                }
                                            });
                                        },
                                        on_delete: move |id: i64| {
                                            match storage::delete_project(id) {
                                                Ok(()) => {
                                                    projects.with_mut(|items| items.retain(|item| item.id != id));
                                                    notice.set(Some("Projeto removido da biblioteca.".to_string()));
                                                }
                                                Err(error) => notice.set(Some(format!("Não foi possível remover o projeto: {error}"))),
                                            }
                                        },
                                    }
                                }
                            }
                        }

                        div { class: "roadmap-note",
                            span { "✳" }
                            div {
                                strong { "Fluxo por commit" }
                                p { "Abra um projeto clonado, selecione um commit e crie um preview isolado. O build será servido somente em localhost." }
                            }
                        }
                    }
                } else {
                    section { class: "content activity-content",
                        div { class: "page-heading",
                            div { div { class: "eyebrow", "HISTÓRICO LOCAL" } h1 { "Atividade" } p { "Eventos de sincronização, builds e deploys aparecerão aqui quando essas fases forem implementadas." } }
                        }
                        div { class: "empty-state compact-empty",
                            div { class: "empty-symbol", "◷" }
                            h3 { "Nenhuma atividade ainda" }
                            p { "Esta área será conectada ao histórico de tarefas nas próximas etapas." }
                        }
                    }
                }
            }
        }

        if show_new_project() {
            NewProjectDialog {
                on_close: move |_| show_new_project.set(false),
                on_create: move |project: NewProject| {
                    match storage::add_project(project) {
                        Ok(created) => {
                            projects.with_mut(|items| items.push(created.clone()));
                            show_new_project.set(false);
                            notice.set(Some(format!("Carregando {}…", created.name)));
                            spawn(async move {
                                let project = created.clone();
                                let outcome = tokio::task::spawn_blocking(move || gitops::clone_repository(&project.repo_url, &project.default_branch, std::path::Path::new(&project.local_path))).await;
                                match outcome {
                                    Ok(Ok(())) => { selected_project.set(Some(created)); selected_nav.set("Deployments".into()); notice.set(None); }
                                    Ok(Err(error)) => notice.set(Some(format!("Não foi possível clonar. Conecte sua conta ou confira a URL: {error}"))),
                                    Err(error) => notice.set(Some(error.to_string())),
                                }
                            });
                        }
                        Err(error) => {
                            show_new_project.set(false);
                            notice.set(Some(format!("Não foi possível salvar o projeto: {error}")));
                        }
                    }
                },
            }
        }
        if show_connections() {
            ConnectionsDialog { on_close: move |_| show_connections.set(false) }
        }
    }
}

#[component]
fn ProjectDetail(project: Project, tab: String, on_back: EventHandler<MouseEvent>) -> Element {
    let mut revision = use_signal(|| 0_u64);
    let mut selected_commit = use_signal(|| None::<String>);
    let mut sync_notice = use_signal(|| None::<String>);
    let mut show_preview = use_signal(|| false);
    let mut selected_log = use_signal(|| None::<storage::BuildRecord>);
    let mut promotion_candidate = use_signal(|| None::<storage::BuildRecord>);
    let mut show_runner_setup = use_signal(|| false);
    let mut commit_state = use_signal(Vec::<gitops::Commit>::new);
    let mut commits_loading = use_signal(|| true);
    let scan_path = project.local_path.clone();
    let scan_branch = project.default_branch.clone();
    use_effect(move || {
        let _ = revision();
        let scan_path = scan_path.clone();
        let scan_branch = scan_branch.clone();
        commits_loading.set(true);
        spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let path = std::path::Path::new(&scan_path);
                if path.join(".git").exists() {
                    gitops::commits(path, 50, &scan_branch)
                } else {
                    Ok(Vec::new())
                }
            })
            .await;
            match result {
                Ok(Ok(commits)) => commit_state.set(commits),
                Ok(Err(error)) => {
                    sync_notice.set(Some(format!("Não foi possível ler os commits: {error}")))
                }
                Err(error) => sync_notice.set(Some(format!(
                    "A leitura dos commits foi interrompida: {error}"
                ))),
            }
            commits_loading.set(false);
        });
    });
    let automatic_sync_path = project.local_path.clone();
    use_future(move || {
        let automatic_sync_path = automatic_sync_path.clone();
        async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(
                    preferences::load().sync_seconds.clamp(15, 300),
                ))
                .await;
                let path = std::path::PathBuf::from(&automatic_sync_path);
                if !path.join(".git").exists() {
                    continue;
                }
                let outcome = tokio::task::spawn_blocking(move || gitops::fetch(&path)).await;
                if matches!(outcome, Ok(Ok(()))) {
                    revision.set(revision() + 1);
                }
            }
        }
    });
    let root = std::path::Path::new(&project.local_path);
    let is_cloned = root.join(".git").exists();
    let commits = commit_state();
    let stack = if is_cloned {
        Some(detector::analyze(root))
    } else {
        None
    };
    let branch = project.default_branch.clone();
    let title = project.name.clone();
    let local_path = project.local_path.clone();
    let stack_summary = stack
        .as_ref()
        .map(|i| i.summary())
        .unwrap_or_else(|| "Não identificado".into());
    let manager = stack
        .as_ref()
        .and_then(|i| i.package_manager.clone())
        .unwrap_or_else(|| "Não detectado".into());
    let node_version = stack
        .as_ref()
        .and_then(|i| i.node_version.clone())
        .unwrap_or_else(|| "Não declarada".into());
    let lockfile_status = stack
        .as_ref()
        .map(|i| i.reproducibility())
        .unwrap_or("Não analisado");
    let build_status = if stack.as_ref().is_some_and(|i| i.build_script.is_some()) {
        "Script declarado; não executado"
    } else {
        "Script não detectado"
    };
    let test_status = if stack.as_ref().is_some_and(|i| i.test_script.is_some()) {
        "Script declarado; não executado"
    } else {
        "Não configurados"
    };
    let services_status = match stack.as_ref() {
        Some(i) if i.has_supabase => "Supabase detectado",
        Some(i) if i.has_compose => "Docker Compose detectado",
        Some(_) => "Nenhum serviço detectado",
        None => "Não analisado",
    };
    let builds = storage::list_builds(project.id).unwrap_or_default();
    let active_production = storage::active_production(project.id).ok().flatten();
    let runner_info = if tab == "Configurações" {
        runner::inspect(&project).ok()
    } else {
        None
    };
    let project_for_preview = project.clone();
    let project_for_open_after_build = project.clone();
    rsx! {
        section { class: "content project-detail",
            button { class: "back-button", onclick: move |event| on_back.call(event), "← Voltar à biblioteca" }
            div { class: "page-heading detail-heading",
                div {
                    div { class: "eyebrow", "PROJETO LOCAL · {project.repo_owner}/{project.repo_name}" }
                    h1 { "{tab}" }
                    p { "{title} · {branch}" }
                }
                if is_cloned {
                    button { class: "secondary-button", onclick: move |_| {
                        let path = std::path::PathBuf::from(&local_path);
                        sync_notice.set(Some("Sincronizando referências em segundo plano…".into()));
                        spawn(async move {
                            let result = tokio::task::spawn_blocking(move || gitops::fetch(&path)).await;
                            match result {
                                Ok(Ok(())) => { revision.set(revision() + 1); sync_notice.set(Some("Referências remotas atualizadas.".into())); }
                                Ok(Err(error)) => sync_notice.set(Some(format!("Falha na sincronização: {error}"))),
                                Err(error) => sync_notice.set(Some(format!("Sincronização interrompida: {error}"))),
                            }
                        });
                    }, "Sincronizar commits" }
                }
            }
            if let Some(message) = sync_notice() { div { class: "notice detail-notice", "{message}" } }
            if !is_cloned {
                div { class: "notice", "Este projeto ainda não foi clonado. Volte à biblioteca para iniciar o clone manualmente." }
            } else {
                if tab == "Configurações" {
                ux::EnvironmentPanel { project_id: project.id }
                button { class: "secondary-button", onclick: move |_| {
                    sync_notice.set(Some("Preparando Git e Node.js em segundo plano…".into()));
                    spawn(async move {
                        let outcome = tokio::task::spawn_blocking(process::install_prerequisites).await;
                        sync_notice.set(Some(match outcome {
                            Ok(Ok(())) => "Ferramentas preparadas. Reabra o LocalDeploy para atualizar o ambiente.".into(),
                            Ok(Err(error)) => error,
                            Err(error) => error.to_string(),
                        }));
                    });
                }, "Preparar ferramentas e Docker" }
                div { class: "stats-row detail-stats",
                    div { class: "stat-card", div { class: "stat-label", "COMMITS LOCAIS" }, div { class: "stat-value", "{commits.len()}" }, div { class: "stat-caption", "Até 50 commits recentes" } }
                    div { class: "stat-card", div { class: "stat-label", "TECNOLOGIAS DETECTADAS" }, div { class: "stat-value stat-placeholder", "{stack_summary}" }, div { class: "stat-caption", "Análise de arquivos; nenhum script executado" } }
                    div { class: "stat-card", div { class: "stat-label", "PRODUÇÃO LOCAL" }, div { class: "stat-value stat-placeholder", "{active_production.as_ref().and_then(|build| build.endpoint.as_deref()).unwrap_or(\"Não ativa\")}" }, div { class: "stat-caption", "Endpoint estável em loopback" } }
                }
                if stack.is_some() {
                    div { class: "detail-panel",
                        h2 { "Requisitos detectados" }
                        div { class: "requirements-grid",
                            div { strong { "Gerenciador" } span { "{manager}" } }
                            div { strong { "Versão Node" } span { "{node_version}" } }
                            div { strong { "Lockfile" } span { "{lockfile_status}" } }
                            div { strong { "Build" } span { "{build_status}" } }
                            div { strong { "Testes" } span { "{test_status}" } }
                            div { strong { "Serviços" } span { "{services_status}" } }
                        }
                    }
                }
                if let Some(runner_info) = runner_info.as_ref() {
                    div { class: "detail-panel",
                        div { class: "section-toolbar",
                            div { class: "section-title", h2 { "GitHub Actions" } }
                            span { class: "detail-muted", "Runner self-hosted opcional" }
                        }
                        p { class: "detail-muted",
                            match runner_info.status {
                                runner::RunnerStatus::NotInstalled => "Nenhum runner configurado para este repositório.",
                                runner::RunnerStatus::Registered => "Runner registrado e parado.",
                                runner::RunnerStatus::Running => "Runner registrado e em execução.",
                            }
                        }
                        if let Some(name) = runner_info.name.as_ref() { p { class: "detail-muted", "Nome: {name}" } }
                        div { class: "build-actions",
                            button { class: "secondary-button", onclick: { let project = project.clone(); move |_| match runner::generate_workflow(&project) {
                                Ok(()) => sync_notice.set(Some("Workflow criado em .github/workflows/localdeploy.yml. Faça commit e push para ativar os builds do runner neste PC.".into())),
                                Err(error) => sync_notice.set(Some(error)),
                            } }, "Criar workflow local" }
                            button { class: "secondary-button", onclick: { let project = project.clone(); move |_| {
                                let project = project.clone();
                                spawn(async move {
                                    let result = tokio::task::spawn_blocking(move || runner::adapt_workflows(&project)).await;
                                    sync_notice.set(Some(match result { Ok(Ok(report)) => report, Ok(Err(error)) => error, Err(error) => error.to_string() }));
                                });
                            } }, "Adaptar workflows Windows" }
                            match runner_info.status {
                                runner::RunnerStatus::NotInstalled => rsx! {
                                    button { class: "secondary-button", onclick: move |_| show_runner_setup.set(true), "Configurar runner" }
                                },
                                runner::RunnerStatus::Registered => {
                                    let directory = runner_info.directory.clone();
                                    rsx! { button { class: "secondary-button", onclick: move |_| match runner::start(&directory) {
                                        Ok(()) => { revision.set(revision() + 1); sync_notice.set(Some("Runner iniciado.".into())); },
                                        Err(error) => sync_notice.set(Some(error)),
                                    }, "Iniciar runner" } }
                                },
                                runner::RunnerStatus::Running => {
                                    let directory = runner_info.directory.clone();
                                    rsx! { button { class: "history-toggle danger-action", onclick: move |_| match runner::stop(&directory) {
                                        Ok(()) => { revision.set(revision() + 1); sync_notice.set(Some("Runner encerrado.".into())); },
                                        Err(error) => sync_notice.set(Some(error)),
                                    }, "Parar runner" } }
                                },
                            }
                        }
                    }
                }
                }
                if tab == "Deployments" {
                div { class: "detail-panel commits-panel",
                    div { class: "section-toolbar",
                        div { class: "section-title", h2 { "Deployments" }, span { class: "count-badge", "{commits.len()}" } }
                        span { class: "detail-muted", "Commit · resultado do build · ambiente" }
                    }
                    div { class: "deployment-table-header",
                        span { "Commit" }
                        span { "Estado" }
                        span { "Ambiente" }
                        span { "SHA" }
                        span { "Ação" }
                    }
                    if commits_loading() {
                        div { class: "detail-empty loading-inline", span { class: "spinner" } "Carregando histórico e referências…" }
                    } else if commits.is_empty() {
                        div { class: "detail-empty", "Nenhum commit local encontrado." }
                    } else {
                        for commit in commits.iter() {
                            div { class: if selected_commit().as_deref() == Some(commit.sha.as_str()) { "detail-commit selected" } else { "detail-commit" }, key: "{commit.sha}", onclick: { let sha = commit.sha.clone(); move |_| selected_commit.set(Some(sha.clone())) },
                                div { class: "detail-commit-main", strong { "{commit.subject}" } span { "{commit.author}" } }
                                div { class: "detail-commit-meta", code { "{commit.short_sha}" } span { "{commit.date}" }
                                    if let Some(production) = active_production.as_ref().filter(|build| build.commit_sha == commit.sha) {
                                        span { class: "ready-status", "ready" }
                                        button { class: "environment-button production", onclick: { let endpoint = production.endpoint.clone(); move |_| {
                                            if let Some(endpoint) = endpoint.as_ref() {
                                                match webbrowser::open(endpoint) {
                                                    Ok(()) => sync_notice.set(Some(format!("Produção local aberta em {endpoint}."))),
                                                    Err(error) => sync_notice.set(Some(format!("Não foi possível abrir a produção local: {error}"))),
                                                }
                                            }
                                        } }, "Produção" }
                                    } else if let Some(build) = builds.iter().find(|build| build.environment == "preview" && build.commit_sha == commit.sha) {
                                        span { class: if build.status == "ready" { "ready-status" } else { "pending-status" }, "{build.status}" }
                                        if build.status == "ready" {
                                            button { class: "environment-button", onclick: { let project = project.clone(); let build = build.clone(); move |_| {
                                                let project = project.clone();
                                                let build = build.clone();
                                                sync_notice.set(Some("Abrindo teste local…".into()));
                                                spawn(async move { match tokio::task::spawn_blocking(move || executor::start_preview(&project, &build)).await.unwrap_or_else(|error| Err(error.to_string())) {
                                                Ok(started) => {
                                                    if let Some(endpoint) = started.endpoint.as_ref() {
                                                        match webbrowser::open(endpoint) {
                                                            Ok(()) => sync_notice.set(Some(format!("Commit aberto em {endpoint}."))),
                                                            Err(error) => sync_notice.set(Some(format!("Teste iniciado, mas não foi possível abrir o navegador: {error}"))),
                                                        }
                                                    }
                                                    revision.set(revision() + 1);
                                                }
                                                Err(error) => sync_notice.set(Some(error)),
                                            } revision.set(revision() + 1); });
                                            } }, {commit.location.label()} }
                                        } else {
                                            button { class: "environment-button failed", onclick: { let sha = commit.sha.clone(); move |_| { selected_commit.set(Some(sha.clone())); show_preview.set(true); } }, {commit.location.label()} }
                                        }
                                        if std::path::Path::new(&build.workspace_path).exists() {
                                            button { class: "delete-workspace", title: "Apagar arquivos temporários deste commit", onclick: { let project = project.clone(); let build = build.clone(); move |_| {
                                                sync_notice.set(Some("Apagando arquivos temporários em segundo plano…".into()));
                                                let project = project.clone();
                                                let build = build.clone();
                                                spawn(async move {
                                                    let result = tokio::task::spawn_blocking(move || executor::delete_preview_workspace(&project, &build)).await;
                                                    match result {
                                                        Ok(Ok(())) => { revision.set(revision() + 1); sync_notice.set(Some("Arquivos temporários removidos.".into())); },
                                                        Ok(Err(error)) => sync_notice.set(Some(error)),
                                                        Err(error) => sync_notice.set(Some(format!("A limpeza foi interrompida: {error}"))),
                                                    }
                                                });
                                            } }, "×" }
                                        }
                                    } else {
                                        span { class: "pending-status", "não construído" }
                                        button { class: "environment-button", onclick: { let sha = commit.sha.clone(); move |_| { selected_commit.set(Some(sha.clone())); show_preview.set(true); } }, {commit.location.label()} }
                                    }
                                }
                            }
                        }
                    }
                }
                }
                if tab == "Logs" {
                div { class: "detail-panel builds-panel",
                    div { class: "section-toolbar",
                        div { class: "section-title", h2 { "Execuções e logs" }, span { class: "count-badge", "{builds.len()}" } }
                        span { class: "detail-muted", "Builds e previews persistidos neste dispositivo" }
                    }
                    if builds.is_empty() {
                        div { class: "detail-empty", "Nenhum preview criado para este projeto." }
                    } else {
                        for build in builds.iter() {
                            div { class: "build-row", key: "{build.id}",
                                div { class: "build-main",
                                    strong { "{build.environment} · {build.status}" }
                                    span { "{build.commit_sha.chars().take(12).collect::<String>()} · {build.started_at}" }
                                    if let Some(endpoint) = build.endpoint.as_ref() { code { "{endpoint}" } }
                                    if let Some(error) = build.error_summary.as_ref() { span { class: "build-error", "{error}" } }
                                }
                                div { class: "build-actions",
                                    if build.status == "ready" && build.environment == "preview" {
                                        button { class: "history-toggle", onclick: { let project = project.clone(); let build = build.clone(); move |_| {
                                            match executor::start_preview(&project, &build) {
                                                Ok(started) => {
                                                    if let Some(endpoint) = started.endpoint.as_ref() {
                                                        match webbrowser::open(endpoint) {
                                                            Ok(()) => sync_notice.set(Some(format!("Preview aberto em {endpoint}."))),
                                                            Err(error) => sync_notice.set(Some(format!("Preview iniciado, mas não foi possível abrir o navegador: {error}"))),
                                                        }
                                                    }
                                                    revision.set(revision() + 1);
                                                }
                                                Err(error) => sync_notice.set(Some(error)),
                                            }
                                        } }, "Preview" }
                                    } else if let Some(endpoint) = build.endpoint.as_ref() {
                                        button { class: "history-toggle", onclick: { let endpoint = endpoint.clone(); move |_| {
                                            match webbrowser::open(&endpoint) {
                                                Ok(()) => sync_notice.set(Some("Endpoint aberto no navegador padrão.".into())),
                                                Err(error) => sync_notice.set(Some(format!("Não foi possível abrir o navegador: {error}"))),
                                            }
                                        } }, "Abrir" }
                                    }
                                    button { class: "history-toggle", onclick: { let build = build.clone(); move |_| selected_log.set(Some(build.clone())) }, "Ver log" }
                                    if build.status == "ready" && build.process_id.is_some() {
                                        button { class: "history-toggle danger-action", onclick: { let build = build.clone(); move |_| {
                                            match executor::stop_preview(&build) {
                                                Ok(()) => { revision.set(revision() + 1); sync_notice.set(Some("Preview encerrado.".into())); }
                                                Err(error) => sync_notice.set(Some(error)),
                                            }
                                        } }, "Parar" }
                                    }
                                    if build.status == "ready" && build.environment == "preview" {
                                        button { class: "history-toggle", onclick: { let build = build.clone(); move |_| promotion_candidate.set(Some(build.clone())) }, "Promover" }
                                    }
                                }
                            }
                        }
                    }
                }
                }
                if tab == "Configurações" && active_production.is_some() {
                    button { class: "secondary-button rollback-button", onclick: { let project = project.clone(); move |_| {
                        match executor::rollback_production(&project) {
                            Ok(_) => { revision.set(revision() + 1); sync_notice.set(Some("Rollback concluído em http://127.0.0.1:4000.".into())); }
                            Err(error) => sync_notice.set(Some(error)),
                        }
                    } }, "Reverter produção local" }
                }
            }
        }
        if show_preview() {
            if let Some(sha) = selected_commit() {
                PreviewDialog {
                    project: project_for_preview.clone(),
                    commit_sha: sha,
                    on_close: move |_| show_preview.set(false),
                    on_created: { let project = project_for_open_after_build.clone(); move |build| {
                        let project = project.clone();
                        show_preview.set(false);
                        sync_notice.set(Some("Build aprovado. Iniciando o localhost…".into()));
                        spawn(async move {
                            let result = tokio::task::spawn_blocking(move || executor::start_preview(&project, &build)).await;
                            match result {
                                Ok(Ok(started)) => {
                                    if let Some(endpoint) = started.endpoint.as_ref() {
                                        match webbrowser::open(endpoint) {
                                            Ok(()) => sync_notice.set(Some(format!("Commit aprovado e aberto em {endpoint}."))),
                                            Err(error) => sync_notice.set(Some(format!("Commit aprovado, mas o navegador não abriu: {error}"))),
                                        }
                                    }
                                }
                                Ok(Err(error)) => sync_notice.set(Some(error)),
                                Err(error) => sync_notice.set(Some(format!("A inicialização foi interrompida: {error}"))),
                            }
                            revision.set(revision() + 1);
                        });
                    } },
                }
            }
        }
        if let Some(build) = selected_log() {
            LogDialog { record: build, on_close: move |_| selected_log.set(None) }
        }
        if let Some(build) = promotion_candidate() {
            PromotionDialog {
                project: project.clone(),
                preview: build,
                on_close: move |_| promotion_candidate.set(None),
                on_promoted: move |result: Result<String, String>| {
                    revision.set(revision() + 1);
                    promotion_candidate.set(None);
                    sync_notice.set(Some(match result {
                        Ok(message) => message,
                        Err(error) => error,
                    }));
                },
            }
        }
        if show_runner_setup() {
            RunnerSetupDialog {
                project: project.clone(),
                on_close: move |_| show_runner_setup.set(false),
                on_finished: move |outcome: Result<String, String>| {
                    revision.set(revision() + 1);
                    show_runner_setup.set(false);
                    sync_notice.set(Some(match outcome { Ok(message) => message, Err(error) => error }));
                },
            }
        }
    }
}

#[component]
fn RunnerSetupDialog(
    project: Project,
    on_close: EventHandler<MouseEvent>,
    on_finished: EventHandler<Result<String, String>>,
) -> Element {
    let mut personal_token = use_signal(String::new);
    let mut approved = use_signal(|| false);
    let mut result = use_signal(|| None::<String>);
    let mut loading = use_signal(|| false);
    let can_register = approved() && !personal_token().trim().is_empty() && !loading();
    rsx! {
        div { class: "modal-backdrop",
            section { class: "modal preview-modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "runner-title",
                div { class: "modal-heading",
                    div {
                        div { class: "eyebrow", "GITHUB ACTIONS" }
                        h2 { id: "runner-title", "Configurar runner neste PC" }
                        p { "O runner oficial Windows x64 será baixado e registrado somente para {project.repo_owner}/{project.repo_name}." }
                    }
                    button { class: "icon-button close-button", aria_label: "Fechar", disabled: loading(), onclick: move |event| on_close.call(event), "×" }
                }
                if loading() {
                    div { class: "loading-build", span { class: "spinner large" } h3 { "Configurando runner" } p { "Baixando e registrando o runner oficial em segundo plano. Nenhuma janela de terminal será aberta." } }
                }
                if let Some(message) = result() { div { class: "form-error", "{message}" } }
                div { class: "modal-info", span { "ⓘ" } "Informe um token GitHub com permissão para administrar Actions neste repositório. O token é usado apenas para pedir um token temporário de registro; o LocalDeploy não o salva." }
                label { class: "form-field",
                    span { "Token de acesso do GitHub" }
                    input {
                        r#type: "password",
                        autocomplete: "off",
                        value: "{personal_token}",
                        oninput: move |event| personal_token.set(event.value()),
                    }
                }
                label { class: "approval-row",
                    input { r#type: "checkbox", checked: approved(), onchange: move |_| approved.toggle() }
                    span { "Autorizo baixar e registrar o runner oficial neste computador. Workflows deste repositório poderão executar código aqui." }
                }
                div { class: "modal-actions",
                    button { class: "secondary-button", disabled: loading(), onclick: move |event| on_close.call(event), "Cancelar" }
                    button { class: "primary-button", disabled: !can_register, onclick: move |_| {
                        loading.set(true);
                        result.set(None);
                        let project = project.clone();
                        let token = personal_token();
                        spawn(async move {
                            let outcome = tokio::task::spawn_blocking(move || runner::install_register_and_start(&project, &token)).await;
                            loading.set(false);
                            match outcome {
                                Ok(Ok(_)) => on_finished.call(Ok("Runner GitHub Actions registrado e iniciado.".into())),
                                Ok(Err(error)) => result.set(Some(error)),
                                Err(error) => result.set(Some(format!("A configuração foi interrompida: {error}"))),
                            }
                        });
                    }, "Baixar e iniciar runner" }
                }
            }
        }
    }
}

#[component]
fn PreviewDialog(
    project: Project,
    commit_sha: String,
    on_close: EventHandler<MouseEvent>,
    on_created: EventHandler<storage::BuildRecord>,
) -> Element {
    let mut trusted = use_signal(|| false);
    let mut run_tests = use_signal(|| preferences::load().run_tests);
    let mut result = use_signal(|| None::<String>);
    let mut loading = use_signal(|| false);
    let plan = executor::plan(&project, &commit_sha);
    let progress_workspace = plan.as_ref().ok().map(|plan| plan.workspace.clone());
    let cancel_workspace = progress_workspace.clone();
    let mut progress = use_signal(|| "Carregando snapshot do commit…".to_string());
    use_future(move || {
        let workspace = progress_workspace.clone();
        async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if loading() {
                    if let Some(workspace) = workspace.as_ref() {
                        if let Ok(stage) =
                            std::fs::read_to_string(workspace.join(".localdeploy-progress"))
                        {
                            progress.set(stage);
                        }
                    }
                }
            }
        }
    });
    let can_run = trusted() && plan.is_ok() && !loading();
    rsx! {
        div { class: "modal-backdrop",
            section { class: "modal preview-modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "preview-title",
                div { class: "modal-heading",
                    div {
                        div { class: "eyebrow", "BUILD ISOLADO" }
                        h2 { id: "preview-title", "Preparar commit" }
                        p { "Vamos preparar este commit e abrir o teste quando estiver pronto." }
                    }
                    button { class: "icon-button close-button", aria_label: "Fechar", disabled: loading(), onclick: move |event| on_close.call(event), "×" }
                }
                if loading() {
                    div { class: "loading-build",
                        span { class: "spinner large" }
                        h3 { "Preparando este commit" }
                        p { "{progress}" }
                        button { class: "secondary-button", onclick: move |_| {
                            if let Some(workspace) = cancel_workspace.as_ref() {
                                match std::fs::write(workspace.join(".localdeploy-cancel"), "cancel") {
                                    Ok(()) => progress.set("Cancelando…".into()),
                                    Err(error) => result.set(Some(format!("Aguarde o carregamento do snapshot para cancelar: {error}"))),
                                }
                            }
                        }, "Cancelar execução" }
                        p { "Baixando o snapshot, verificando dependências, executando testes e gerando o build. A interface continuará disponível." }
                    }
                }
                if let Some(message) = result() { div { class: "form-error", "{message}" } }
                match plan {
                    Ok(plan) => rsx! {
                        details {
                        summary { "Ver plano de preparação" }
                        div { class: "execution-plan",
                            div { strong { "Commit" } code { "{plan.commit_sha}" } }
                            div { strong { "Workspace" } span { "{plan.workspace.display()}" } }
                            div { strong { "Gerenciador" } span { "{plan.package_manager}" } }
                            div { strong { "Instalação" } span { "{plan.install_action}" } }
                            div { strong { "Build" } span { "{plan.build_action}" } }
                            div { strong { "Testes" } span { "{plan.test_action}" } }
                            div { strong { "Preview" } span { "{plan.preview_action}" } }
                        }
                        div { class: "plan-warnings",
                            for warning in plan.warnings { div { "⚠ {warning}" } }
                        }
                        }
                        label { class: "approval-row",
                            input { r#type: "checkbox", checked: trusted(), onchange: move |_| trusted.toggle() }
                            span { "Confio neste repositório e aprovo a execução dos scripts listados." }
                        }
                        label { class: "approval-row",
                            input { r#type: "checkbox", checked: run_tests(), onchange: move |_| run_tests.toggle() }
                            span { "Executar testes quando o projeto declarar um script test." }
                        }
                        div { class: "modal-actions",
                            button { class: "secondary-button", disabled: loading(), onclick: move |event| on_close.call(event), "Cancelar" }
                            button {
                                class: "primary-button",
                                disabled: !can_run,
                                onclick: move |_| {
                                    loading.set(true);
                                    result.set(None);
                                    let project = project.clone();
                                    let commit_sha = commit_sha.clone();
                                    let should_run_tests = run_tests();
                                    spawn(async move {
                                        let outcome = tokio::task::spawn_blocking(move || executor::prepare_preview(&project, &commit_sha, should_run_tests)).await;
                                        loading.set(false);
                                        match outcome {
                                            Ok(Ok(build)) => on_created.call(build),
                                            Ok(Err(error)) => result.set(Some(error)),
                                            Err(error) => result.set(Some(format!("A preparação foi interrompida: {error}"))),
                                        }
                                    });
                                },
                                "Executar build"
                            }
                        }
                    },
                    Err(error) => rsx! {
                        div { class: "form-error", "Não é possível criar este preview: {error}" }
                        div { class: "modal-actions", button { class: "secondary-button", onclick: move |event| on_close.call(event), "Fechar" } }
                    }
                }
            }
        }
    }
}

#[component]
fn LogDialog(record: storage::BuildRecord, on_close: EventHandler<MouseEvent>) -> Element {
    let content = executor::read_log(&record).unwrap_or_else(|error| error);
    rsx! {
        div { class: "modal-backdrop",
            section { class: "modal log-modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "log-title",
                div { class: "modal-heading",
                    div { div { class: "eyebrow", "LOG LOCAL" } h2 { id: "log-title", "Build {record.id}" } p { "{record.log_path}" } }
                    button { class: "icon-button close-button", aria_label: "Fechar", onclick: move |event| on_close.call(event), "×" }
                }
                pre { class: "log-output", "{content}" }
                div { class: "modal-actions", button { class: "secondary-button", onclick: move |event| on_close.call(event), "Fechar" } }
            }
        }
    }
}

#[component]
fn PromotionDialog(
    project: Project,
    preview: storage::BuildRecord,
    on_close: EventHandler<MouseEvent>,
    on_promoted: EventHandler<Result<String, String>>,
) -> Element {
    rsx! {
        div { class: "modal-backdrop",
            section { class: "modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "promotion-title",
                div { class: "modal-heading",
                    div {
                        div { class: "eyebrow", "PRODUÇÃO LOCAL" }
                        h2 { id: "promotion-title", "Promover preview" }
                        p { "O commit {preview.commit_sha.chars().take(12).collect::<String>()} será verificado antes da troca e publicado apenas em http://127.0.0.1:4000." }
                    }
                    button { class: "icon-button close-button", aria_label: "Fechar", onclick: move |event| on_close.call(event), "×" }
                }
                div { class: "modal-info", span { "ⓘ" } "A produção local não fica exposta na rede. Se a versão candidata falhar no health check, a versão anterior é reiniciada." }
                div { class: "modal-actions",
                    button { class: "secondary-button", onclick: move |event| on_close.call(event), "Cancelar" }
                    button { class: "primary-button", onclick: move |_| {
                        let project = project.clone();
                        let preview = preview.clone();
                        spawn(async move {
                            let outcome = tokio::task::spawn_blocking(move || executor::promote_preview(&project, &preview)).await
                                .unwrap_or_else(|error| Err(error.to_string()))
                                .map(|_| "Produção local disponível em http://127.0.0.1:4000.".to_string());
                            on_promoted.call(outcome);
                        });
                    }, "Promover" }
                }
            }
        }
    }
}

#[component]
fn ProjectCard(
    project: Project,
    is_cloned: bool,
    on_open: EventHandler<Project>,
    on_clone: EventHandler<Project>,
    on_delete: EventHandler<i64>,
) -> Element {
    let mut menu_open = use_signal(|| false);
    let clone_project = project.clone();
    let open_project = project.clone();
    let branch_label = project.default_branch.clone();
    let stack = if is_cloned {
        Some(detector::analyze(std::path::Path::new(&project.local_path)))
    } else {
        None
    };
    rsx! {
        article { class: "project-card",
            div { class: "card-topline",
                div { class: "project-icon", "{project.name.chars().next().unwrap_or('P').to_ascii_uppercase()}" }
                div { class: "card-menu-wrap",
                    button { class: "icon-button", "aria-label": "Ações do projeto", onclick: move |_| menu_open.toggle(), "···" }
                    if menu_open() {
                        div { class: "dropdown",
                            button { onclick: move |_| on_delete.call(project.id), "Remover da biblioteca" }
                        }
                    }
                }
            }
            h3 { "{project.name}" }
            p { class: "repo-name", "{project.repo_owner}/{project.repo_name}" }
            button { class: "open-project", onclick: move |_| on_open.call(open_project.clone()), "Abrir projeto →" }
            if let Some(info) = stack.as_ref() {
                div { class: "stack-summary", "{info.summary()}" }
            }
            div { class: "card-divider" }
            div { class: "card-meta-row",
                div { class: "card-meta", span { class: "meta-icon", "⌘" } span { "Branch padrão" } }
                span { class: "branch-tag", "{branch_label}" }
            }
            div { class: "card-meta-row",
                div { class: "card-meta", span { class: "meta-icon", "▱" } span { "Pasta local" } }
                span { class: "path-value", title: "{project.local_path}", "{project.local_path}" }
            }
            div { class: "card-bottom",
                span { class: if is_cloned { "ready-status" } else { "pending-status" }, span { class: if is_cloned { "ready-dot" } else { "pending-dot" } } if is_cloned { "Clonado · pronto para commits" } else { "Ainda não clonado" } }
                span { class: "card-date", "adicionado {project.created_at}" }
            }
            if !is_cloned {
                button { class: "clone-button", onclick: move |_| on_clone.call(clone_project.clone()), "Clonar repositório" }
            }
        }
    }
}

#[component]
fn NewProjectDialog(
    on_close: EventHandler<MouseEvent>,
    on_create: EventHandler<NewProject>,
) -> Element {
    let mut repo_url = use_signal(String::new);
    let mut branch = use_signal(|| "main".to_string());
    let mut local_path = use_signal(String::new);
    let mut validation_error = use_signal(|| None::<String>);

    let suggested_path = storage::app_data_dir()
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join("LocalDeploy"))
        .join("projects");

    rsx! {
        div { class: "modal-backdrop",
            section { class: "modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "new-project-title",
                div { class: "modal-heading",
                    div {
                        div { class: "eyebrow", "BIBLIOTECA LOCAL" }
                        h2 { id: "new-project-title", "Adicionar projeto" }
                        p { "Registre os dados do repositório e escolha onde ele ficará no dispositivo." }
                    }
                    button { class: "icon-button close-button", aria_label: "Fechar", onclick: move |event| on_close.call(event), "×" }
                }
                if let Some(error) = validation_error() { div { class: "form-error", "{error}" } }
                label { class: "form-field",
                    span { "URL do repositório GitHub" }
                    input {
                        placeholder: "https://github.com/organizacao/repositorio",
                        value: "{repo_url}",
                        oninput: move |event| repo_url.set(event.value()),
                    }
                    small { "O clone usa o Git instalado no dispositivo. Repositórios privados precisam de credenciais do GitHub." }
                }
                button { class: "secondary-button", onclick: move |_| match gitops::open_github_sign_in() {
                    Ok(()) => validation_error.set(Some("O login foi aberto pelo Git Credential Manager. Conclua no navegador e volte a esta janela.".into())),
                    Err(error) => validation_error.set(Some(error)),
                }, "Entrar no GitHub pelo navegador" }
                details {
                summary { "Opções avançadas" }
                div { class: "form-row",
                    label { class: "form-field",
                        span { "Branch padrão" }
                        input { value: "{branch}", oninput: move |event| branch.set(event.value()) }
                    }
                    label { class: "form-field",
                        span { "Pasta local" }
                        input {
                            placeholder: "{suggested_path.display()}",
                            value: "{local_path}",
                            oninput: move |event| local_path.set(event.value()),
                        }
                    }
                }
                }
                div { class: "modal-info", span { "ⓘ" } "Vamos clonar o repositório e abrir a lista de commits. A execução de testes só começa quando você solicitar." }
                div { class: "modal-actions",
                    button { class: "secondary-button", onclick: move |event| on_close.call(event), "Cancelar" }
                    button {
                        class: "primary-button",
                        onclick: move |_| {
                            let path = if local_path().trim().is_empty() {
                                suggested_path.join(storage::repo_name_from_url(&repo_url()).unwrap_or_else(|_| "projeto".to_string()))
                            } else {
                                std::path::PathBuf::from(local_path().trim())
                            };
                            match NewProject::new(repo_url().trim(), branch().trim(), path) {
                                Ok(project) => { validation_error.set(None); on_create.call(project); }
                                Err(error) => validation_error.set(Some(error.to_string())),
                            }
                        },
                        "Criar e abrir projeto"
                    }
                }
            }
        }
    }
}
