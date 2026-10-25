use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StackInfo {
    pub technologies: Vec<String>,
    pub package_manager: Option<String>,
    pub node_version: Option<String>,
    pub build_script: Option<String>,
    pub test_script: Option<String>,
    pub dev_script: Option<String>,
    pub start_script: Option<String>,
    pub has_lockfile: bool,
    pub has_supabase: bool,
    pub has_compose: bool,
}

impl StackInfo {
    pub fn summary(&self) -> String {
        if self.technologies.is_empty() {
            "Tecnologia não identificada".into()
        } else {
            self.technologies.join(" · ")
        }
    }

    pub fn reproducibility(&self) -> &'static str {
        if self.has_lockfile {
            "Lockfile encontrado"
        } else if self
            .technologies
            .iter()
            .any(|t| t.contains("Node.js") || t.contains("Next.js") || t.contains("Vue"))
        {
            "Sem lockfile · versões não fixadas"
        } else {
            "Lockfile não detectado"
        }
    }
}

pub fn analyze(root: &Path) -> StackInfo {
    let exists = |name: &str| root.join(name).exists();
    let package = std::fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    let mut technologies = Vec::new();
    let mut package_manager = None;
    let mut node_version = None;
    let mut build_script = None;
    let mut test_script = None;
    let mut dev_script = None;
    let mut start_script = None;
    let has_lockfile = exists("Cargo.lock")
        || [
            "package-lock.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "bun.lock",
            "bun.lockb",
        ]
        .iter()
        .any(|name| exists(name));

    if let Some(manifest) = package {
        technologies.push("Node.js".into());
        let deps = manifest.get("dependencies").and_then(|v| v.as_object());
        let dev = manifest.get("devDependencies").and_then(|v| v.as_object());
        let has_dep = |name: &str| {
            deps.is_some_and(|d| d.contains_key(name)) || dev.is_some_and(|d| d.contains_key(name))
        };
        if has_dep("next")
            || exists("next.config.js")
            || exists("next.config.mjs")
            || exists("next.config.ts")
        {
            technologies.push("Next.js".into());
        } else if has_dep("vite")
            || exists("vite.config.js")
            || exists("vite.config.ts")
            || exists("vite.config.mjs")
        {
            technologies.push("Vite".into());
        }
        if has_dep("vue") {
            technologies.push("Vue".into());
        }
        if has_dep("react") {
            technologies.push("React".into());
        }
        if has_dep("typescript") || exists("tsconfig.json") {
            technologies.push("TypeScript".into());
        }
        package_manager = manifest
            .get("packageManager")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| {
                if exists("pnpm-lock.yaml") {
                    Some("pnpm".into())
                } else if exists("yarn.lock") {
                    Some("yarn".into())
                } else if exists("bun.lock") || exists("bun.lockb") {
                    Some("bun".into())
                } else if exists("package-lock.json") {
                    Some("npm".into())
                } else {
                    None
                }
            });
        node_version = manifest
            .get("engines")
            .and_then(|v| v.get("node"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        build_script = manifest
            .get("scripts")
            .and_then(|v| v.get("build"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        test_script = manifest
            .get("scripts")
            .and_then(|v| v.get("test"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        dev_script = manifest
            .get("scripts")
            .and_then(|v| v.get("dev"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        start_script = manifest
            .get("scripts")
            .and_then(|v| v.get("start"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);
    }

    if exists("Cargo.toml") {
        technologies.push(if exists("Dioxus.toml") {
            "Rust · Dioxus".into()
        } else {
            "Rust".into()
        });
        if let Ok(raw) = std::fs::read_to_string(root.join("Cargo.toml")) {
            if let Ok(manifest) = toml::from_str::<toml::Value>(&raw) {
                if let Some(name) = manifest
                    .get("package")
                    .and_then(|p| p.get("name"))
                    .and_then(toml::Value::as_str)
                {
                    technologies.push(format!("Cargo: {name}"));
                }
            }
        }
    }
    if exists("supabase/config.toml") || exists("supabase") {
        technologies.push("Supabase".into());
    }
    if exists("compose.yaml") || exists("compose.yml") || exists("docker-compose.yml") {
        technologies.push("Docker Compose".into());
    }

    StackInfo {
        technologies,
        package_manager,
        node_version,
        build_script,
        test_script,
        dev_script,
        start_script,
        has_lockfile,
        has_supabase: exists("supabase") || exists("supabase/config.toml"),
        has_compose: exists("compose.yaml")
            || exists("compose.yml")
            || exists("docker-compose.yml"),
    }
}
