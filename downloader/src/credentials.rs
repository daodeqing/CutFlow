//! Windows user-bound DPAPI credential storage, never part of a project.
use crate::{Result, io};
use std::{fs, path::Path};

pub fn save(root: &Path, cookie: &str) -> Result<()> {
    let cookie = cookie.trim();
    if cookie.len() > 16384 || cookie.contains(['\r', '\n', '\0']) {
        return Err("Cookie 应为单行且不超过 16 KiB".into());
    }
    if !cookie.split(';').any(|p| {
        p.trim()
            .strip_prefix("SESSDATA=")
            .is_some_and(|v| !v.trim().is_empty())
    }) {
        return Err("Cookie 缺少非空 SESSDATA".into());
    }
    let sealed = protect(cookie.as_bytes(), true)?;
    fs::create_dir_all(root).map_err(io)?;
    // Encrypted bytes only. Do not put cookie into a task, log or command line.
    fs::write(root.join("account.dpapi"), sealed).map_err(io)
}

pub fn load(root: &Path) -> Result<Option<String>> {
    let path = root.join("account.dpapi");
    if !path.exists() {
        return Ok(None);
    }
    let sealed = fs::read(path).map_err(io)?;
    let plain = protect(&sealed, false)?;
    String::from_utf8(plain)
        .map(Some)
        .map_err(|_| "凭据内容损坏，请重新设置".into())
}

pub fn clear(root: &Path) -> Result<()> {
    match fs::remove_file(root.join("account.dpapi")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(e)),
    }
}

#[cfg(not(windows))]
pub fn protect(_: &[u8], _: bool) -> Result<Vec<u8>> {
    Err("本版本的凭据存储仅支持 Windows DPAPI；未保存明文凭据".into())
}

#[cfg(windows)]
pub fn protect(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    use std::ptr::{null, null_mut};
    #[repr(C)]
    struct Blob {
        size: u32,
        data: *mut u8,
    }
    #[link(name = "crypt32")]
    unsafe extern "system" {
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
    unsafe extern "system" {
        fn LocalFree(ptr: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    let input = Blob {
        size: u32::try_from(bytes.len()).map_err(io)?,
        data: bytes.as_ptr() as *mut _,
    };
    let mut output = Blob {
        size: 0,
        data: null_mut(),
    };
    // CRYPTPROTECT_UI_FORBIDDEN; deliberately no LOCAL_MACHINE flag.
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                null(),
                null(),
                null_mut(),
                null_mut(),
                1,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                null_mut(),
                null(),
                null_mut(),
                null_mut(),
                1,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err("Windows 无法保护或读取凭据，请在原 Windows 用户下重新设置".into());
    }
    let value = unsafe { std::slice::from_raw_parts(output.data, output.size as usize).to_vec() };
    unsafe {
        LocalFree(output.data.cast());
    }
    Ok(value)
}
