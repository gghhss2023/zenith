use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const INTEGRATION_SCRIPT: &str = include_str!("../resources/shell-integration.sh");

pub fn install_to(dir: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join("shell-integration.sh");
    let up_to_date = fs::read_to_string(&path)
        .map(|cur| cur == INTEGRATION_SCRIPT)
        .unwrap_or(false);
    if !up_to_date {
        fs::write(&path, INTEGRATION_SCRIPT)?;
    }
    Ok(path)
}

pub fn ensure_installed() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    install_to(&PathBuf::from(home).join(".config/zenith")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("zenith_shellint_{}_{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn install_writes_script() {
        let dir = temp_dir("write");
        let path = install_to(&dir).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), INTEGRATION_SCRIPT);
    }

    #[test]
    fn install_restores_modified_script() {
        let dir = temp_dir("restore");
        let path = install_to(&dir).unwrap();
        fs::write(&path, "tampered").unwrap();
        let path2 = install_to(&dir).unwrap();
        assert_eq!(path, path2);
        assert_eq!(fs::read_to_string(&path).unwrap(), INTEGRATION_SCRIPT);
    }
}
