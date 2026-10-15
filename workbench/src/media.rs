use crate::{Error, Result};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub fn copy_into_project(project: &Path, folder: &Path, source: &Path) -> Result<PathBuf> {
    let source = source.canonicalize()?;
    let project = project.canonicalize()?;
    if !source.is_file() {
        return Err(Error::new("invalid_media", "素材必须是文件"));
    }
    if source.starts_with(&project) {
        return Ok(source);
    }
    std::fs::create_dir_all(folder)?;
    let folder = folder.canonicalize()?;
    if !folder.starts_with(&project) {
        return Err(Error::new("invalid_media", "素材目录必须位于工程内"));
    }
    let mut input = std::fs::File::open(&source)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let digest = format!("{:x}", digest.finalize());
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("media");
    let target = folder.join(format!("{digest}.{extension}"));
    if target.exists() {
        let mut existing = std::fs::File::open(&target)?;
        let mut actual = Sha256::new();
        loop {
            let count = existing.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            actual.update(&buffer[..count]);
        }
        if format!("{:x}", actual.finalize()) != digest {
            return Err(Error::new("invalid_media", "工程素材已变化，拒绝覆盖"));
        }
        return Ok(target);
    }
    input.seek(SeekFrom::Start(0))?;
    let partial = folder.join(format!("{}.part", uuid::Uuid::new_v4()));
    let mut output = std::fs::File::create(&partial)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    drop(output);
    std::fs::rename(partial, &target)?;
    Ok(target)
}
