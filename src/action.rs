//! Filesystem operations to stage files.

use std::fmt;
use std::fs;
use std::path;

use error;

// `Display` is required for dry-runs / previews.
/// Operation for setting up staged directory tree.
pub trait FsAction: fmt::Display + fmt::Debug {
    /// Execute the current action, writing to the stage.
    fn perform(&self) -> Result<(), error::StagingError>;
}

/// Specifies a staged directory to be created.
#[derive(Clone, Debug)]
pub struct CreateDirectory {
    staged: path::PathBuf,
}

impl CreateDirectory {
    /// Specifies a staged directory to be created.
    ///
    /// - `staged`: full path to future directory.
    pub fn new<P>(staged: P) -> Self
    where
        P: Into<path::PathBuf>,
    {
        Self {
            staged: staged.into(),
        }
    }

    /// The directory that will be created.
    pub fn dir(&self) -> &path::Path {
        &self.staged
    }
}

impl fmt::Display for CreateDirectory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "mkdir {:?}", self.staged)
    }
}

impl FsAction for CreateDirectory {
    fn perform(&self) -> Result<(), error::StagingError> {
        fs::create_dir_all(&self.staged)
            .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }
}

/// Specifies a file to be staged into the target directory.
#[derive(Clone, Debug)]
pub struct CopyFile {
    staged: path::PathBuf,
    source: path::PathBuf,
}

impl CopyFile {
    /// Specifies a file to be staged into the target directory.
    ///
    /// - `staged`: full path to future file.
    /// - `source`: full path to file being written to `staged`.
    pub fn new<D, S>(staged: D, source: S) -> Self
    where
        D: Into<path::PathBuf>,
        S: Into<path::PathBuf>,
    {
        Self {
            staged: staged.into(),
            source: source.into(),
        }
    }

    /// The file to be copied.
    pub fn source(&self) -> &path::Path {
        &self.source
    }

    /// The file's destination path
    pub fn destination(&self) -> &path::Path {
        &self.staged
    }
}

impl fmt::Display for CopyFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "cp {:?} {:?}", self.source, self.staged)
    }
}

impl FsAction for CopyFile {
    fn perform(&self) -> Result<(), error::StagingError> {
        if let Some(parent) = self.staged.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;
        }
        fs::copy(&self.source, &self.staged)
            .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }
}

/// Specifies a symbolic link file to be staged into the target directory.
#[derive(Clone, Debug)]
pub struct Symlink {
    staged: path::PathBuf,
    target: path::PathBuf,
}

impl Symlink {
    /// Specifies a symbolic link file to be staged into the target directory.
    ///
    /// - `staged`: full path for future symlink.
    /// - `target`: path that symlink will point to.
    pub fn new<S, T>(staged: S, target: T) -> Self
    where
        S: Into<path::PathBuf>,
        T: Into<path::PathBuf>,
    {
        Self {
            staged: staged.into(),
            target: target.into(),
        }
    }

    /// The path where the link will exist.
    pub fn link(&self) -> &path::Path {
        &self.staged
    }

    /// The location the link points to.
    pub fn target(&self) -> &path::Path {
        &self.target
    }
}

impl fmt::Display for Symlink {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ln -s {:?} {:?}", self.target, self.staged)
    }
}

impl FsAction for Symlink {
    fn perform(&self) -> Result<(), error::StagingError> {
        if let Some(parent) = self.staged.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;
        }
        #[allow(deprecated)]
        fs::soft_link(&self.staged, &self.target)
            .map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;

        Ok(())
    }
}
