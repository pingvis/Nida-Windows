//! Persistent Windows application diagnostics. Keep stderr working for shells
//! and launch probes while also writing a bounded, rotating file.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn path(given: Option<&OsStr>, local_data: Option<&Path>) -> Option<PathBuf> {
    given
        .map(PathBuf::from)
        .or_else(|| local_data.map(|root| root.join("Nida").join("logs").join("nida.log")))
}

pub struct WindowsLog {
    file: RotatingFile,
}

impl WindowsLog {
    /// Open the application log, preserving previous contents until rotation.
    ///
    /// # Errors
    /// Returns directory, open or rotation failures without changing stderr.
    pub fn open(path: &Path) -> io::Result<Self> {
        Ok(Self {
            file: RotatingFile::open(path, super::log_file::LOG_CAP)?,
        })
    }
}

impl Write for WindowsLog {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // Shell redirection, CI and the startup benchmark still see messages.
        let _ = io::stderr().write_all(bytes);
        self.file.write_all(bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

struct RotatingFile {
    path: PathBuf,
    file: Option<File>,
    bytes: u64,
    cap: u64,
}

impl RotatingFile {
    fn open(path: &Path, cap: u64) -> io::Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            path: path.to_path_buf(),
            file: Some(file),
            bytes,
            cap,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        // Windows needs the old file closed before it is replaced/reopened.
        self.file.take();
        let mut old = self.path.as_os_str().to_owned();
        old.push(".old");
        let old = PathBuf::from(old);
        let result = (|| {
            match std::fs::remove_file(&old) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            std::fs::rename(&self.path, old)
        })();
        // Reopen even when rotation fails, so a transient failure does not
        // permanently disable file logging for the rest of the show.
        self.file = Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?,
        );
        self.bytes = self.file.as_ref().expect("reopened").metadata()?.len();
        result
    }
}

impl Write for RotatingFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes > 0 && self.bytes.saturating_add(bytes.len() as u64) > self.cap {
            self.rotate()?;
        }
        let Some(file) = self.file.as_mut() else {
            return Err(io::Error::other("log file is unavailable"));
        };
        let written = file.write(bytes)?;
        self.bytes = self.bytes.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.as_mut().map_or(Ok(()), Write::flush)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_local_data_and_honours_explicit_unicode_path() {
        assert_eq!(
            path(None, Some(Path::new("local"))),
            Some(PathBuf::from("local/Nida/logs/nida.log"))
        );
        let explicit = OsStr::new("Žurnalai/įvykis.log");
        assert_eq!(path(Some(explicit), None), Some(PathBuf::from(explicit)));
        assert_eq!(path(None, None), None);
    }

    #[test]
    fn appends_and_rotates_repeatedly_during_a_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Žurnalai/nida.log");
        let old = path.with_file_name("nida.log.old");
        {
            let mut log = RotatingFile::open(&path, 8).unwrap();
            log.write_all(b"first\n").unwrap();
        }
        let mut log = RotatingFile::open(&path, 8).unwrap();
        log.write_all(b"2\n").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first\n2\n");
        log.write_all(b"second\n").unwrap();
        assert_eq!(std::fs::read(&old).unwrap(), b"first\n2\n");
        log.write_all(b"third\n").unwrap();
        log.flush().unwrap();
        assert_eq!(std::fs::read(&old).unwrap(), b"second\n");
        assert_eq!(std::fs::read(&path).unwrap(), b"third\n");
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            2
        );
    }

    #[test]
    fn reports_unwritable_target_without_panicking() {
        let dir = tempfile::tempdir().unwrap();
        assert!(WindowsLog::open(dir.path()).is_err());
    }
}
