use std::process::Command;
thread_local! { static TOOL_PATHS: std::cell::RefCell<Vec<std::path::PathBuf>> = const { std::cell::RefCell::new(Vec::new()) }; }
pub fn set_tool_paths(paths: Vec<std::path::PathBuf>) {
    TOOL_PATHS.with(|current| *current.borrow_mut() = paths);
}

pub fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let program = program.as_ref();
    #[cfg(target_os = "windows")]
    let mut command = if ["npm", "pnpm", "yarn"]
        .iter()
        .any(|name| program == std::ffi::OsStr::new(name))
    {
        let mut command = Command::new("cmd.exe");
        command.args(["/D", "/C"]);
        command.arg(program);
        command
    } else {
        Command::new(program)
    };
    #[cfg(not(target_os = "windows"))]
    let mut command = Command::new(program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    TOOL_PATHS.with(|paths| {
        let mut combined = paths.borrow().clone();
        combined.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        if let Ok(path) = std::env::join_paths(combined) {
            command.env("PATH", path);
        }
    });
    command
}

pub fn install_prerequisites() -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        Err("Preparação automática disponível apenas no Windows.".into())
    }
    #[cfg(target_os = "windows")]
    {
        for (tool, package) in [
            ("git", "Git.Git"),
            ("node", "OpenJS.NodeJS.LTS"),
            ("docker", "Docker.DockerDesktop"),
            ("rustup", "Rustlang.Rustup"),
        ] {
            if hidden_command(tool)
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success())
            {
                continue;
            }
            let status = hidden_command("winget")
                .args(["install", "--id", package, "--exact", "--source", "winget", "--silent", "--accept-source-agreements", "--accept-package-agreements", "--disable-interactivity"])
                .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                .status().map_err(|e| format!("Não foi possível preparar {tool}. O Instalador de Aplicativos do Windows precisa disponibilizar winget: {e}"))?;
            if !status.success() {
                return Err(format!(
                    "A instalação de {tool} falhou. Código: {:?}",
                    status.code()
                ));
            }
        }
        Ok(())
    }
}
