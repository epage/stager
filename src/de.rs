use std::collections::BTreeMap;
use std::path;

use failure;
use liquid;

use builder;

pub trait Render {
    type Rendered;

    fn format(&self, engine: &TemplateEngine) -> Result<Self::Rendered, failure::Error>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Staging(BTreeMap<Template, Vec<Source>>);

impl Render for Staging {
    type Rendered = builder::Staging;

    fn format(&self, engine: &TemplateEngine) -> Result<builder::Staging, failure::Error> {
        let staging: Result<BTreeMap<_, _>, _> = self.0
            .iter()
            .map(|(target, sources)| {
                let target = path::PathBuf::from(target.format(engine)?);
                let sources: &Vec<Source> = sources;
                let sources: Result<Vec<_>, _> =
                    sources.into_iter().map(|s| s.format(engine)).collect();
                sources.map(|s| (target, s))
            })
            .collect();
        staging
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Source {
    Directory(Directory),
    SourceFile(SourceFile),
    SourceFiles(SourceFiles),
    Symlink(Symlink),
}

impl Render for Source {
    type Rendered = Box<builder::ActionBuilder>;

    fn format(
        &self,
        engine: &TemplateEngine,
    ) -> Result<Box<builder::ActionBuilder>, failure::Error> {
        let value: Box<builder::ActionBuilder> = match *self {
            Source::Directory(ref b) => Box::new(b.format(engine)?),
            Source::SourceFile(ref b) => Box::new(b.format(engine)?),
            Source::SourceFiles(ref b) => Box::new(b.format(engine)?),
            Source::Symlink(ref b) => Box::new(b.format(engine)?),
        };
        Ok(value)
    }
}

/// Override the default settings for the target directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Directory {
    access: OneOrMany<Access>,
}

impl Render for Directory {
    type Rendered = builder::Directory;

    fn format(&self, engine: &TemplateEngine) -> Result<builder::Directory, failure::Error> {
        let access = self.access.format(engine)?;
        let value = builder::Directory { access };
        Ok(value)
    }
}

/// Specifies a file to be staged into the target directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    ///  Specifies the full path of the file to be copied into the target directory
    path: Template,
    /// Specifies the name the target file should be renamed as when copying from the source file.
    /// Default is the filename of the source file.
    #[serde(default)]
    rename: Option<Template>,
    /// Specifies symbolic links to `rename` in the same target directory and using the same
    /// `access`.
    #[serde(default)]
    symlink: Option<OneOrMany<Template>>,
    #[serde(default)]
    access: Option<OneOrMany<Access>>,
}

impl Render for SourceFile {
    type Rendered = builder::SourceFile;

    fn format(&self, engine: &TemplateEngine) -> Result<builder::SourceFile, failure::Error> {
        let access = self.access
            .as_ref()
            .map(|a| a.format(engine))
            .map_or(Ok(None), |r| r.map(Some))?
            .unwrap_or_default();
        let symlink = self.symlink
            .as_ref()
            .map(|a| a.format(engine))
            .map_or(Ok(None), |r| r.map(Some))?
            .unwrap_or_default();
        let value = builder::SourceFile {
            path: path::PathBuf::from(self.path.format(engine)?),
            rename: self.rename
                .as_ref()
                .map(|t| t.format(engine))
                .map_or(Ok(None), |r| r.map(Some))?,
            symlink,
            access,
        };
        Ok(value)
    }
}

/// Specifies a collection of files to be staged into the target directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFiles {
    ///  Specifies the root path that `patterns` will be run on to identify files to be copied into
    ///  the target directory.
    path: Template,
    /// Specifies the pattern for executing the recursive/multifile match.
    pattern: OneOrMany<Template>,
    #[serde(default)]
    follow_links: bool,
    #[serde(default)]
    access: Option<OneOrMany<Access>>,
}

impl Render for SourceFiles {
    type Rendered = builder::SourceFiles;

    fn format(&self, engine: &TemplateEngine) -> Result<builder::SourceFiles, failure::Error> {
        let pattern = self.pattern.format(engine)?;
        let access = self.access
            .as_ref()
            .map(|a| a.format(engine))
            .map_or(Ok(None), |r| r.map(Some))?
            .unwrap_or_default();
        let value = builder::SourceFiles {
            path: path::PathBuf::from(self.path.format(engine)?),
            pattern,
            follow_links: self.follow_links,
            access,
        };
        Ok(value)
    }
}

/// Specifies a symbolic link file to be staged into the target directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Symlink {
    /// The literal path for the target to point to.
    target: Template,
    /// Specifies the name the symlink should be given.
    /// Default is the filename of the `target`.
    rename: Template,
    #[serde(default)]
    access: Option<OneOrMany<Access>>,
}

impl Render for Symlink {
    type Rendered = builder::Symlink;

    fn format(&self, engine: &TemplateEngine) -> Result<builder::Symlink, failure::Error> {
        let access = self.access
            .as_ref()
            .map(|a| a.format(engine))
            .map_or(Ok(None), |r| r.map(Some))?
            .unwrap_or_default();
        let value = builder::Symlink {
            target: path::PathBuf::from(self.target.format(engine)?),
            rename: self.rename.format(engine)?,
            access,
        };
        Ok(value)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Access {
    /// Specifies  permissions to be applied to the file.
    op: String,
}

impl Render for Access {
    type Rendered = builder::Access;

    fn format(&self, _engine: &TemplateEngine) -> Result<builder::Access, failure::Error> {
        let value = builder::Access {
            op: self.op.clone(),
        };
        Ok(value)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> Render for OneOrMany<T>
where
    T: Render,
{
    type Rendered = Vec<T::Rendered>;

    fn format(&self, engine: &TemplateEngine) -> Result<Self::Rendered, failure::Error> {
        match *self {
            OneOrMany::One(ref v) => {
                let u = v.format(engine)?;
                Ok(vec![u])
            }
            OneOrMany::Many(ref v) => {
                let u: Result<Vec<_>, _> = v.iter().map(|a| a.format(engine)).collect();
                u
            }
        }
    }
}

// TODO(epage): Look into making template system pluggable
// - Leverage traits
// - Possibly get liquid to also work with serializables like Tera(?)
// But should we?  Would it be better to have consistency in syntax and functionality?
// Either way, might be better to switch to another template engine if it looks like its getting
// traction within Rust community (like whatever is used for cargo templates) and to one that will
// be 1.0 sooner.
pub struct TemplateEngine {
    pub parser: liquid::Parser,
    pub data: liquid::Object,
}

impl TemplateEngine {
    pub fn new(data: liquid::Object) -> Result<Self, failure::Error> {
        // TODO(eage): Better customize liquid
        // - Add raw block
        // - Remove irrelevant filters (like HTML ones)
        // - Add path manipulation filters
        let parser = liquid::ParserBuilder::new().liquid_filters().build();
        Ok(Self { parser, data })
    }

    pub fn render(&self, template: &str) -> Result<String, failure::Error> {
        // TODO(epage): get liquid to be compatible with failure::Fail
        let template = self.parser.parse(template)?;
        let content = template.render(&self.data)?;
        Ok(content)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Template(String);

impl Render for Template {
    type Rendered = String;

    fn format(&self, engine: &TemplateEngine) -> Result<String, failure::Error> {
        engine.render(&self.0)
    }
}
