use crate::preferences::{self, Preferences};

#[component]
pub fn EnvironmentPanel(project_id: i64) -> Element {
    let mut values = use_signal(String::new);
    let mut message = use_signal(String::new);
    let mut reveal = use_signal(|| false);
    rsx! {
        section { class: "detail-panel",
            h2 { "Variáveis do ambiente de teste" }
            p { "Cadastre as configurações da aplicação e do Supabase. Os valores são protegidos pela sua conta do Windows. Salvar substitui o conjunto anterior." }
            label { class: "form-field", span { "Uma variável por linha: NOME=valor" }
                textarea { class: if reveal() { "environment-editor" } else { "environment-editor masked" }, value: "{values}", placeholder: "SUPABASE_URL=…", spellcheck: "false", oninput: move |event| values.set(event.value()) }
            }
            label { class: "approval-row", input { r#type: "checkbox", checked: reveal(), onchange: move |_| reveal.toggle() } "Mostrar valores durante a edição" }
            button { class: "primary-button", onclick: move |_| match crate::secrets::save(project_id, &values()) {
                Ok(()) => { values.set(String::new()); message.set("Variáveis protegidas e salvas. Serão aplicadas ao próximo build.".into()); },
                Err(error) => message.set(error),
            }, "Salvar variáveis" }
            if !message().is_empty() { p { role: "status", "{message}" } }
        }
    }
}
use dioxus::prelude::*;

#[component]
pub fn PreferencesPanel(value: Preferences, on_saved: EventHandler<Preferences>) -> Element {
    let mut draft = use_signal(|| value);
    let mut message = use_signal(String::new);
    let mut preparing = use_signal(|| false);
    rsx! {
        section { class: "content settings-page",
            h1 { "Preferências" }
            p { "Personalize a aparência e o comportamento do LocalDeploy." }
            section { class: "detail-panel settings-grid",
                h2 { "Aparência" }
                label { class: "form-field", span { "Tamanho do texto" }
                    select { value: "{draft().font_size}", onchange: move |event| { if let Ok(size) = event.value().parse() { draft.write().font_size = size; } },
                        for size in [14, 16, 18, 20, 22] { option { value: "{size}", "{size} px" } }
                    }
                }
                label { class: "form-field", span { "Tema" }
                    select { value: "{draft().theme}", onchange: move |event| draft.write().theme = event.value(), option { "Claro" } option { "Escuro" } }
                }
                h2 { "Testes e sincronização" }
                label { class: "form-field", span { "Atualizar os commits a cada" }
                    select { value: "{draft().sync_seconds}", onchange: move |event| { if let Ok(seconds) = event.value().parse() { draft.write().sync_seconds = seconds; } },
                        for seconds in [15, 30, 60, 120, 300] { option { value: "{seconds}", "{seconds} segundos" } }
                    }
                }
                label { class: "form-field", span { "Ambiente padrão" }
                    select { value: "{draft().backend}", onchange: move |event| draft.write().backend = event.value(), option { "Docker" } option { "Nativo" } }
                    small { "Docker separa aplicações web do Windows. Nativo executa projetos Node diretamente com suas permissões." }
                }
                label { class: "approval-row", input { r#type: "checkbox", checked: draft().run_tests, onchange: move |event| draft.write().run_tests = event.checked() } "Executar os testes declarados antes de aprovar um build" }
                button { class: "primary-button", onclick: move |_| match preferences::save(&draft()) {
                    Ok(()) => { on_saved.call(draft()); message.set("Preferências salvas.".into()); },
                    Err(error) => message.set(error),
                }, "Salvar preferências" }
            }
            section { class: "detail-panel",
                h2 { "Preparação do computador" }
                p { "Instale as ferramentas necessárias por um assistente. O Windows pode solicitar permissão ou reinicialização para Docker e WSL2." }
                button { class: "secondary-button", disabled: preparing(), onclick: move |_| {
                    preparing.set(true); message.set("Preparando ferramentas. Aguarde as mensagens do Windows…".into());
                    spawn(async move {
                        let result = tokio::task::spawn_blocking(crate::process::install_prerequisites).await;
                        preparing.set(false);
                        message.set(match result { Ok(Ok(())) => "Preparação concluída. Reabra o aplicativo para atualizar as ferramentas disponíveis.".into(), Ok(Err(error)) => error, Err(error) => error.to_string() });
                    });
                }, if preparing() { "Preparando…" } else { "Preparar ambiente" } }
                button { class: "secondary-button", onclick: move |_| {
                    match crate::storage::app_data_dir().map(|root| root.to_string_lossy().into_owned()) {
                        Ok(root) => { let _ = crate::process::hidden_command("explorer.exe").arg(root).spawn(); },
                        Err(error) => message.set(error.to_string()),
                    }
                }, "Abrir pasta de dados" }
            }
            if !message().is_empty() { div { class: "notice", role: "status", "{message}" } }
        }
    }
}

#[component]
pub fn ConnectionsDialog(on_close: EventHandler<MouseEvent>) -> Element {
    let mut message = use_signal(String::new);
    let mut connecting = use_signal(|| false);
    rsx! {
        div { class: "modal-backdrop",
            section { class: "modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "connections-title",
                div { class: "modal-heading", h2 { id: "connections-title", "Conexões" }
                    button { class: "icon-button", onclick: move |event| on_close.call(event), "×" }
                }
                p { "A autenticação acontece no navegador. O LocalDeploy não pede sua senha." }
                section { class: "connection-card",
                    h3 { "GitHub" }
                    p { "Autorize o acesso para clonar e sincronizar seus repositórios privados." }
                    button { class: "primary-button", disabled: connecting(), onclick: move |_| {
                        connecting.set(true); message.set("Abrindo autenticação segura no navegador…".into());
                        spawn(async move {
                            let outcome = tokio::task::spawn_blocking(crate::gitops::open_github_sign_in).await;
                            connecting.set(false);
                            message.set(match outcome { Ok(Ok(())) => "Conclua o login no navegador e volte ao LocalDeploy. As credenciais serão gerenciadas pelo Git do Windows.".into(), Ok(Err(error)) => error, Err(error) => error.to_string() });
                        });
                    }, "Conectar GitHub" }
                }
                section { class: "connection-card",
                    h3 { "Supabase" }
                    p { "Acesse seus projetos no navegador. O login do painel não configura automaticamente as chaves da aplicação." }
                    button { class: "secondary-button", onclick: move |_| match webbrowser::open("https://supabase.com/dashboard") {
                        Ok(()) => message.set("Painel Supabase aberto. Configure as variáveis do projeto antes de testar.".into()),
                        Err(error) => message.set(error.to_string()),
                    }, "Abrir Supabase" }
                }
                if !message().is_empty() { div { class: "notice", role: "status", "{message}" } }
            }
        }
    }
}
