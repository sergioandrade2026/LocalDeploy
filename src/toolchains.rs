use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

/// Installs a verified Node distribution in application storage. Selection is
/// repeated for every commit; the system installation is never replaced.
pub fn prepare(
    workspace: &Path,
    requirement: Option<&str>,
    bun: bool,
) -> Result<Vec<PathBuf>, String> {
    let tools = crate::storage::app_data_dir()
        .map_err(|e| e.to_string())?
        .join("toolchains");
    let mut paths = vec![tools.join("packages"), tools.join("packages/bin")];
    if cfg!(target_os = "windows") {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|e| e.to_string())?;
        let declared = [".nvmrc", ".node-version"]
            .iter()
            .find_map(|file| fs::read_to_string(workspace.join(file)).ok());
        let requested = declared
            .as_deref()
            .map(str::trim)
            .or(requirement)
            .unwrap_or("*");
        let requested = requested.trim_start_matches('v');
        let requested = if requested == "node" || requested.starts_with("lts/") {
            "*"
        } else {
            requested
        };
        let exact = declared.is_some() && semver::Version::parse(requested).is_ok();
        let normalized = requested
            .split("||")
            .map(str::trim)
            .map(|part| part.split_whitespace().collect::<Vec<_>>().join(", "))
            .collect::<Vec<_>>();
        let ranges = normalized
            .iter()
            .map(|part| {
                semver::VersionReq::parse(&if exact {
                    format!("={part}")
                } else {
                    part.clone()
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Versão Node não compreendida ({requested}): {e}"))?;
        let releases: serde_json::Value = client
            .get("https://nodejs.org/dist/index.json")
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;
        let version = releases
            .as_array()
            .ok_or("Lista de versões Node inválida")?
            .iter()
            .filter(|r| requested != "*" || r["lts"].is_string())
            .filter_map(|r| r["version"].as_str())
            .find(|version| {
                semver::Version::parse(version.trim_start_matches('v'))
                    .is_ok_and(|v| ranges.iter().any(|r| r.matches(&v)))
            })
            .ok_or("Nenhuma versão Node compatível encontrada")?;
        let name = format!("node-{version}-win-x64");
        let directory = tools.join(&name);
        if !directory.join("node.exe").is_file() {
            let file = format!("{name}.zip");
            let base = format!("https://nodejs.org/dist/{version}");
            let sums = client
                .get(format!("{base}/SHASUMS256.txt"))
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string())?
                .text()
                .map_err(|e| e.to_string())?;
            let expected = sums
                .lines()
                .find_map(|line| {
                    let fields = line.split_whitespace().collect::<Vec<_>>();
                    (fields.len() == 2 && fields[1] == file).then(|| fields[0].to_owned())
                })
                .ok_or("Checksum oficial do Node ausente")?;
            let bytes = client
                .get(format!("{base}/{file}"))
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| e.to_string())?
                .bytes()
                .map_err(|e| e.to_string())?;
            if format!("{:x}", Sha256::digest(&bytes)) != expected {
                return Err("Integridade do Node inválida".into());
            }
            fs::create_dir_all(&tools).map_err(|e| e.to_string())?;
            zip::ZipArchive::new(std::io::Cursor::new(bytes))
                .map_err(|e| e.to_string())?
                .extract(&tools)
                .map_err(|e| e.to_string())?;
        }
        paths.insert(0, directory);
    }
    if bun {
        let prefix = tools.join("packages");
        fs::create_dir_all(&prefix).map_err(|e| e.to_string())?;
        crate::process::set_tool_paths(paths.clone());
        let status = crate::process::hidden_command("npm")
            .args(["install", "--global", "--prefix"])
            .arg(&prefix)
            .arg("bun")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Não foi possível preparar Bun".into());
        }
    }
    Ok(paths)
}
