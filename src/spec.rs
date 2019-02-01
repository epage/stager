//! High-level specification for staging files.

use std::ffi;
use std::fmt;
use std::path;

use globwalk;

use super::Staging;
use error;

/// Create concrete filesystem specs.
pub trait SpecificationBuilder: fmt::Debug {
    /// Create concrete filesystem specs.
    fn resolve(&self, target_dir: &path::Path) -> Result<Box<Specification>, error::Errors>;
}

impl<S: SpecificationBuilder + ?Sized> SpecificationBuilder for Box<S> {
    fn resolve(&self, target_dir: &path::Path) -> Result<Box<Specification>, error::Errors> {
        let spec: &S = &self;
        spec.resolve(target_dir)
    }
}

/// Concrete filesystem specs.
pub trait Specification: fmt::Debug {
    /// Apply specification to the stage.
    fn stage(&self, stage: &mut Staging) -> Result<(), error::Errors>;
}

impl<S: Specification + ?Sized> Specification for Box<S> {
    fn stage(&self, stage: &mut Staging) -> Result<(), error::Errors> {
        let spec: &S = &self;
        spec.stage(stage)
    }
}

/// Specifies a file to be staged into the target directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFileBuilder {
    path: path::PathBuf,
    rename: Option<String>,
    symlink: Vec<String>,
}

impl SourceFileBuilder {
    /// Specifies a file to be staged into the target directory.
    ///
    /// - `source`: full path of the file to be copied into the target directory
    pub fn new<P>(source: P) -> Self
    where
        P: Into<path::PathBuf>,
    {
        Self {
            path: source.into(),
            rename: None,
            symlink: Default::default(),
        }
    }

    /// Specifies the name the target file should be renamed as when copying from the source file.
    /// Default is the filename of the source file.
    pub fn rename<S: Into<String>>(mut self, filename: Option<S>) -> Self {
        self.rename = filename.map(|f| f.into());
        self
    }

    /// Specifies symbolic links to `rename` in the same target directory.
    pub fn push_symlinks<I: Iterator<Item = String>>(mut self, symlinks: I) -> Self {
        self.symlink.extend(symlinks);
        self
    }

    /// Resolve a specification for a given `target_dir`.
    pub fn resolve(self, target_dir: &path::Path) -> Result<SourceFile, error::Errors> {
        let SourceFileBuilder {
            path: source,
            rename,
            symlink,
        } = self;

        let dest = {
            let default_name = source.file_name().ok_or_else(|| {
                error::ErrorKind::HarvestingFailed
                    .error()
                    .set_context(format!("SourceFile is missing a filename: {:?}", source))
            })?;
            let dest = rename
                .as_ref()
                .map(|n| ffi::OsStr::new(n))
                .unwrap_or(default_name);
            let dest = path::Path::new(dest);
            if dest.file_name() != Some(dest.as_os_str()) {
                Err(error::ErrorKind::HarvestingFailed
                    .error()
                    .set_context(format!(
                        "SourceFile rename must not change directories: {:?}",
                        dest
                    )))?;
            }
            target_dir.join(dest)
        };

        let symlinks: Result<Vec<_>, error::StagingError> = symlink
            .iter()
            .map(|s| {
                let symlink = path::Path::new(s);
                if symlink.file_name() != Some(symlink.as_os_str()) {
                    Err(error::ErrorKind::HarvestingFailed
                        .error()
                        .set_context(format!(
                            "SourceFile symlink must not change directories: {:?}",
                            dest
                        )))?;
                }
                let symlink = target_dir.join(symlink);
                Ok(symlink)
            })
            .collect();

        let spec = SourceFile {
            source,
            dest,
            symlinks: symlinks?,
        };

        Ok(spec)
    }
}

impl SpecificationBuilder for SourceFileBuilder {
    fn resolve(&self, target_dir: &path::Path) -> Result<Box<Specification>, error::Errors> {
        self.clone().resolve(target_dir).map(|s| {
            let s: Box<Specification> = Box::new(s);
            s
        })
    }
}

/// Specifies a file to be staged into the target directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    source: path::PathBuf,
    dest: path::PathBuf,
    symlinks: Vec<path::PathBuf>,
}

impl Specification for SourceFile {
    fn stage(&self, stage: &mut Staging) -> Result<(), error::Errors> {
        stage.file_from_path(&self.dest, &self.source)?;

        for symlink in &self.symlinks {
            stage.symlink_file(symlink, &self.dest)?;
        }

        Ok(())
    }
}

/// Specifies a collection of files to be staged into the target directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFilesBuilder {
    path: path::PathBuf,
    pattern: Vec<String>,
    follow_links: bool,
    allow_empty: bool,
}

impl SourceFilesBuilder {
    /// Specifies a collection of files to be staged into the target directory.
    ///
    /// - `source`: the root path that `pattern` will be run on to identify files to be copied into
    ///   the target directory.
    pub fn new<P>(source: P) -> Self
    where
        P: Into<path::PathBuf>,
    {
        Self {
            path: source.into(),
            pattern: Default::default(),
            follow_links: false,
            allow_empty: false,
        }
    }

    /// Specifies the `pattern` for executing the recursive/multifile match.
    ///
    /// `pattern` uses [gitignore][gitignore] syntax.
    ///
    /// [gitignore]: https://git-scm.com/docs/gitignore#_pattern_format
    pub fn push_patterns<I: Iterator<Item = String>>(mut self, patterns: I) -> Self {
        self.pattern.extend(patterns);
        self
    }

    /// When true, symbolic links are followed as if they were normal directories and files.
    /// If a symbolic link is broken or is involved in a loop, an error is yielded.
    pub fn follow_links(mut self, yes: bool) -> Self {
        self.follow_links = yes;
        self
    }

    /// Toggles whether no results for the pattern constitutes an error.
    ///
    /// Generally, the default of `false` is best because it makes mistakes more obvious.  An
    /// example of when no results are acceptable is a default staging configuration that
    /// implements a lot of default "good enough" policy.
    pub fn allow_empty(mut self, yes: bool) -> Self {
        self.allow_empty = yes;
        self
    }

    /// Resolve a specification for a given `target_dir`.
    pub fn resolve(self, target_dir: &path::Path) -> Result<SourceFiles, error::Errors> {
        let spec = SourceFiles {
            target_dir: target_dir.to_owned(),
            path: self.path,
            pattern: self.pattern,
            follow_links: self.follow_links,
            allow_empty: self.allow_empty,
        };
        Ok(spec)
    }
}

impl SpecificationBuilder for SourceFilesBuilder {
    fn resolve(&self, target_dir: &path::Path) -> Result<Box<Specification>, error::Errors> {
        self.clone().resolve(target_dir).map(|s| {
            let s: Box<Specification> = Box::new(s);
            s
        })
    }
}

/// Specifies a collection of files to be staged into the target directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFiles {
    target_dir: path::PathBuf,
    path: path::PathBuf,
    pattern: Vec<String>,
    follow_links: bool,
    allow_empty: bool,
}

impl Specification for SourceFiles {
    fn stage(&self, stage: &mut Staging) -> Result<(), error::Errors> {
        let source_root = self.path.as_path();

        let mut empty = true;
        let walker = globwalk::GlobWalker::from_patterns(source_root, &self.pattern)
            .map_err(|e| error::ErrorKind::HarvestingFailed.error().set_cause(e))?
            .follow_links(self.follow_links);
        for entry in walker {
            let entry = entry.map_err(|e| error::ErrorKind::HarvestingFailed.error().set_cause(e))?;
            let source = entry.path();
            if source.is_dir() {
                continue;
            }
            let dest = source
                .strip_prefix(source_root)
                .map_err(|e| error::ErrorKind::HarvestingFailed.error().set_cause(e))?;
            let dest = self.target_dir.join(dest);
            stage.file_from_path(&dest, source)?;
            empty = false;
        }

        if empty {
            if self.allow_empty {
                info!(
                    "No files found under {:?} with patterns {:?}",
                    self.path, self.pattern
                );
            } else {
                Err(error::ErrorKind::HarvestingFailed
                    .error()
                    .set_context(format!(
                        "SourceFiles found no files under {:?} with patterns {:?}",
                        self.path, self.pattern
                    )))?
            }
        }

        Ok(())
    }
}

/// Specifies a symbolic link file to be staged into the target directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymlinkBuilder {
    target: path::PathBuf,
    rename: Option<String>,
}

impl SymlinkBuilder {
    /// Specifies a symbolic link file to be staged into the target directory.
    ///
    /// - `target`: The literal path for the target to point to.
    pub fn new<P>(target: P) -> Self
    where
        P: Into<path::PathBuf>,
    {
        Self {
            target: target.into(),
            rename: None,
        }
    }

    /// Specifies the name the symlink should be given.
    /// Default is the filename of the `target`.
    pub fn rename<S: Into<String>>(mut self, filename: Option<S>) -> Self {
        self.rename = filename.map(|f| f.into());
        self
    }

    /// Resolve a specification for a given `target_dir`.
    pub fn resolve(self, target_dir: &path::Path) -> Result<Symlink, error::Errors> {
        let SymlinkBuilder { target, rename } = self;

        let path = {
            let default_filename = target.file_name().ok_or_else(|| {
                error::ErrorKind::HarvestingFailed
                    .error()
                    .set_context(format!(
                        "Symlink target has no file name: {}",
                        target.display()
                    ))
            })?;
            let filename = rename
                .as_ref()
                .map(|n| ffi::OsStr::new(n))
                .unwrap_or(&default_filename);
            let path = path::Path::new(filename);
            if path.file_name() != Some(path.as_os_str()) {
                Err(error::ErrorKind::HarvestingFailed
                    .error()
                    .set_context(format!(
                        "Symlink rename must not change directories: {:?}",
                        filename,
                    )))?
            }
            target_dir.join(path)
        };

        let spec = Symlink {
            path,
            target: target,
        };

        Ok(spec)
    }
}

impl SpecificationBuilder for SymlinkBuilder{
    fn resolve(&self, target_dir: &path::Path) -> Result<Box<Specification>, error::Errors> {
        self.clone().resolve(target_dir).map(|s| {
            let s: Box<Specification> = Box::new(s);
            s
        })
    }
}

/// Specifies a symbolic link file to be staged into the target directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symlink {
    path: path::PathBuf,
    target: path::PathBuf,
}

impl Specification for Symlink {
    fn stage(&self, stage: &mut Staging) -> Result<(), error::Errors> {
        // TODO: figure out which to use.
        stage.symlink_file(&self.path, &self.target)?;

        Ok(())
    }
}
