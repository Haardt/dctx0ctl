use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(crate) fn sources(path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if path.is_file() {
        files.push(path.to_owned());
    } else {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if !matches!(
                    entry.file_name().to_str(),
                    Some(".dctx0ctl" | ".runatron" | ".git" | "node_modules" | "target")
                ) {
                    sources(&path, files)?;
                }
            } else if path.extension().is_some_and(|ext| ext == "dctx0") {
                files.push(path);
            }
        }
    }
    Ok(())
}
