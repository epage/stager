//! Stage files on a target.

use std::fs;
use std::io;
use std::io::Read;
use std::path;

use super::error;
use super::Staging;

/// A location on the filesystem to stage to.
#[derive(Debug, Clone)]
pub struct Filesystem {
    root: path::PathBuf,
}

impl Filesystem {
    /// A location to stage files within.
    pub fn new<P: Into<path::PathBuf>>(root: P) -> Self {
        Self { root: root.into() }
    }
}

impl Staging for Filesystem {
    fn directory(&mut self, path: &path::Path) -> Result<(), error::StagingError> {
        let target = self.root.join(path);
        fs::create_dir_all(&target)
            .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }

    fn file_from_path(
        &mut self,
        dest: &path::Path,
        src: &path::Path,
    ) -> Result<(), error::StagingError> {
        let target = self.root.join(dest);
        fs::copy(&src, &target).map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }

    fn file_from_reader(
        &mut self,
        dest: &path::Path,
        src: &mut Read,
    ) -> Result<(), error::StagingError> {
        let target = self.root.join(dest);
        let mut f = fs::File::create(target)
            .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;
        io::copy(src, &mut f).map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn symlink_dir(
        &mut self,
        path: &path::Path,
        target: &path::Path,
    ) -> Result<(), error::StagingError> {
        use std::os::windows::fs;
        let path = self.root.join(path);
        fs::symlink_dir(target, &path)
            .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn symlink_file(
        &mut self,
        path: &path::Path,
        target: &path::Path,
    ) -> Result<(), error::StagingError> {
        use std::os::windows::fs;
        let path = self.root.join(path);
        fs::symlink_file(target, &path)
            .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn symlink_dir(
        &mut self,
        path: &path::Path,
        target: &path::Path,
    ) -> Result<(), error::StagingError> {
        use std::os::unix::fs;
        let path = self.root.join(path);
        fs::symlink(target, &path)
            .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn symlink_file(
        &mut self,
        path: &path::Path,
        target: &path::Path,
    ) -> Result<(), error::StagingError> {
        use std::os::unix::fs;
        fs::symlink(target, path)
            .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }
}
