use std::fs;
use std::io::Read;
use std::path;

use error;

/// Build up a staged filesystem.
pub trait Staging {
    /// Create a directory within the stage.
    fn directory(&mut self, path: &path::Path) -> Result<(), error::StagingError>;

    /// Create a file within the stage.
    fn file_from_path(
        &mut self,
        dest: &path::Path,
        src: &path::Path,
    ) -> Result<(), error::StagingError> {
        let mut f =
            fs::File::open(src).map_err(|e| error::ErrorKind::StagingFailed.error().set_cause(e))?;
        self.file_from_reader(dest, &mut f)
    }

    /// Create a file within the stage.
    fn file_from_reader(
        &mut self,
        dest: &path::Path,
        src: &mut Read,
    ) -> Result<(), error::StagingError>;

    /// Create a symlink to a directory within the stage.
    fn symlink_dir(
        &mut self,
        path: &path::Path,
        target: &path::Path,
    ) -> Result<(), error::StagingError>;

    /// Create a symlink to a file within the stage.
    fn symlink_file(
        &mut self,
        path: &path::Path,
        target: &path::Path,
    ) -> Result<(), error::StagingError>;
}
