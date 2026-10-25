use std::path::Path;

#[cfg(target_os = "windows")]
fn protect(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    #[repr(C)]
    struct Blob {
        len: u32,
        data: *mut u8,
    }
    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            input: *const Blob,
            description: *const u16,
            entropy: *const Blob,
            reserved: *mut std::ffi::c_void,
            prompt: *mut std::ffi::c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            input: *const Blob,
            description: *mut *mut u16,
            entropy: *const Blob,
            reserved: *mut std::ffi::c_void,
            prompt: *mut std::ffi::c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(value: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    let input = Blob {
        len: bytes
            .len()
            .try_into()
            .map_err(|_| "Configuração muito grande")?,
        data: bytes.as_ptr() as *mut u8,
    };
    let mut output = Blob {
        len: 0,
        data: std::ptr::null_mut(),
    };
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(format!(
            "O Windows não conseguiu acessar as credenciais protegidas: {}",
            std::io::Error::last_os_error()
        ));
    }
    let result = unsafe { std::slice::from_raw_parts(output.data, output.len as usize).to_vec() };
    unsafe {
        LocalFree(output.data.cast());
    }
    Ok(result)
}
#[cfg(not(target_os = "windows"))]
fn protect(_bytes: &[u8], _encrypt: bool) -> Result<Vec<u8>, String> {
    Err("Proteção de credenciais disponível no Windows.".into())
}

pub fn save(project: i64, content: &str) -> Result<(), String> {
    for line in content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let (key, _) = line
            .split_once('=')
            .ok_or("Use uma variável por linha no formato NOME=valor")?;
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err("Nome de variável inválido".into());
        }
    }
    let root = crate::storage::app_data_dir()
        .map_err(|e| e.to_string())?
        .join("secrets");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join(format!("{project}.bin")),
        protect(content.as_bytes(), true)?,
    )
    .map_err(|e| e.to_string())
}
pub fn materialize(project: i64, workspace: &Path) -> Result<(), String> {
    let path = crate::storage::app_data_dir()
        .map_err(|e| e.to_string())?
        .join("secrets")
        .join(format!("{project}.bin"));
    if !path.exists() {
        return Ok(());
    }
    let bytes = protect(&std::fs::read(path).map_err(|e| e.to_string())?, false)?;
    std::fs::write(workspace.join(".env"), bytes).map_err(|e| e.to_string())
}
