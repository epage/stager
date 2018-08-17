//! High-level requirements for staging files.
//!
//! ## Basic Example
//!
//! ```rust
//! use std::path;
//! use stager::builder;
//! use stager::builder::ActionBuilder;
//!
//! let target = path::Path::new("/tmp/example"); // dummy data
//! let stage = builder::Stage::default(); // dummy data
//! let stage = stage.build(target).unwrap();
//! ```

use std::collections::BTreeMap;
use std::ffi;
use std::fmt;
use std::iter;
use std::path;

use globwalk;

use action;
use error;

/// Create concrete filesystem actions.
pub trait ActionBuilder: fmt::Debug {
    // TODO(epage):
    // - Change to `Iterator`.
    /// Create concrete filesystem actions.
    ///
    /// - `target_dir`: The location everything will be written to (ie the stage).
    fn build(&self, target_dir: &path::Path) -> Result<Vec<Box<action::Action>>, error::Errors>;
}

impl<A: ActionBuilder + ?Sized> ActionBuilder for Box<A> {
    fn build(&self, target_dir: &path::Path) -> Result<Vec<Box<action::Action>>, error::Errors> {
        let target: &A = &self;
        target.build(target_dir)
    }
}

/// For each stage target, a list of sources to populate it with.
///
/// The target is a path relative to the stage root.
#[derive(Default, Debug)]
pub struct Stage(BTreeMap<path::PathBuf, Vec<Box<ActionBuilder>>>);

impl ActionBuilder for Stage {
    fn build(&self, target_dir: &path::Path) -> Result<Vec<Box<action::Action>>, error::Errors> {
        let staging: Result<Vec<_>, _> = self.0
            .iter()
            .map(|(target, sources)| {
                if target.is_absolute() {
                    let mut errors = error::Errors::new();
                    errors.push(
                        error::StagingError::new(error::ErrorKind::HarvestingFailed).set_context(
                            format!("target must be relative to the stage root: {:?}", target),
                        ),
                    );
                    return errors;
                }
                let target = target_dir.join(target);
                let mut errors = error::Errors::new();
                let sources = {
                    let sources = sources.into_iter().flat_map(|s| s.build(&target));
                    let sources = error::ErrorPartition::new(sources, &mut errors);
                    let sources: Vec<_> = sources.collect();
                    sources
                };
                errors.ok(sources)
            })
            .collect();
        let staging = staging?;
        let staging: Vec<_> = staging
            .into_iter()
            .flat_map(|v| v.into_iter().flat_map(|v: Vec<_>| v.into_iter()))
            .collect();
        Ok(staging)
    }
}

impl iter::FromIterator<(path::PathBuf, Vec<Box<ActionBuilder>>)> for Stage {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (path::PathBuf, Vec<Box<ActionBuilder>>)>,
    {
        let staging = iter.into_iter().collect();
        Self { 0: staging }
    }
}

/// Specifies a file to be staged into the target directory.
#[derive(Clone, Debug)]
pub struct SourceFile {
    path: path::PathBuf,
    rename: Option<String>,
    symlink: Vec<String>,
}

impl SourceFile {
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
}

impl ActionBuilder for SourceFile {
    fn build(&self, target_dir: &path::Path) -> Result<Vec<Box<action::Action>>, error::Errors> {
        let path = self.path.as_path();
        if !path.is_absolute() {
            return error::StagingError::new(error::ErrorKind::HarvestingFailed)
                .set_context(format!("SourceFile path must be absolute: {:?}", path));
        }

        let filename = self.rename
            .as_ref()
            .map(|n| ffi::OsStr::new(n))
            .unwrap_or_else(|| path.file_name().unwrap_or_default());
        let filename = path::Path::new(filename);
        if filename.file_name() != Some(filename.as_os_str()) {
            return error::StagingError::new(error::ErrorKind::HarvestingFailed).set_context(
                format!(
                    "SourceFile rename must not change directories: {:?}",
                    filename
                ),
            );
        }
        let copy_target = target_dir.join(filename);
        let copy: Box<action::Action> = Box::new(action::CopyFile::new(&copy_target, path));

        let mut actions = vec![copy];
        actions.extend(self.symlink.iter().map(|s| {
            let s = path::Path::new(s);
            // TODO(epage): Re-enable this error check
            //if s.file_name() != Some(s.as_os_str()) {
            //    bail!("SourceFile symlink must not change directories: {:?}", s);
            //}
            let sym_target = target_dir.join(s);
            let a: Box<action::Action> = Box::new(action::Symlink::new(sym_target, &copy_target));
            a
        }));
        // TODO(epage): Set symlink permissions

        Ok(actions)
    }
}

/// Specifies a collection of files to be staged into the target directory.
#[derive(Clone, Debug)]
pub struct SourceFiles {
    path: path::PathBuf,
    pattern: Vec<String>,
    follow_links: bool,
    allow_empty: bool,
}

impl SourceFiles {
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
}

impl ActionBuilder for SourceFiles {
    fn build(&self, target_dir: &path::Path) -> Result<Vec<Box<action::Action>>, error::Errors> {
        let mut actions: Vec<Box<action::Action>> = Vec::new();
        let source_root = self.path.as_path();
        if !source_root.is_absolute() {
            return error::StagingError::new(error::ErrorKind::HarvestingFailed).set_context(
                format!("SourceFiles path must be absolute: {:?}", source_root),
            );
        }
        for entry in globwalk::GlobWalker::from_patterns(source_root, &self.pattern)?
            .follow_links(self.follow_links)
        {
            let entry = entry?;
            let source_file = entry.path();
            if source_file.is_dir() {
                continue;
            }
            let rel_source = source_file.strip_prefix(source_root)?;
            let copy_target = target_dir.join(rel_source);
            let copy: Box<action::Action> =
                Box::new(action::CopyFile::new(&copy_target, source_file));
            actions.push(copy);
        }

        if actions.is_empty() {
            if self.allow_empty {
                info!(
                    "No files found under {:?} with patterns {:?}",
                    self.path, self.pattern
                );
            } else {
                return error::StagingError::new(error::ErrorKind::HarvestingFailed).set_context(
                    format!(
                        "No files found under {:?} with patterns {:?}",
                        self.path, self.pattern
                    ),
                );
            }
        }

        Ok(actions)
    }
}

/// Specifies a symbolic link file to be staged into the target directory.
#[derive(Clone, Debug)]
pub struct Symlink {
    target: path::PathBuf,
    rename: Option<String>,
}

impl Symlink {
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
}

impl ActionBuilder for Symlink {
    fn build(&self, target_dir: &path::Path) -> Result<Vec<Box<action::Action>>, error::Errors> {
        let target = self.target.as_path();

        let filename = self.rename
            .as_ref()
            .map(|n| ffi::OsStr::new(n))
            .unwrap_or_else(|| target.file_name().unwrap_or_default());
        let filename = path::Path::new(filename);
        if filename.file_name() != Some(filename.as_os_str()) {
            return error::StagingError::new(error::ErrorKind::HarvestingFailed).set_context(
                format!("Symlink rename must not change directories: {:?}", filename),
            );
        }
        let staged = target_dir.join(filename);
        let link: Box<action::Action> = Box::new(action::Symlink::new(&staged, target));

        let actions = vec![link];

        Ok(actions)
    }
}
