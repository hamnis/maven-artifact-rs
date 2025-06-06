use crate::resolver::ResolveError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use thiserror::Error;
use url::Url;

mod artifact;
mod metadata;
mod resolver;

#[derive(Debug, Error)]
pub enum MavenError {
    #[error("Http error")]
    ResolveError(#[from] ResolveError),
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct GroupId(String);

impl GroupId {
    pub fn into_string(self) -> String {
        self.0
    }
    pub fn path_string(&self) -> String {
        self.0.replace(".", "/")
    }
}

impl From<String> for GroupId {
    fn from(value: String) -> Self {
        GroupId(value)
    }
}

impl From<&str> for GroupId {
    fn from(value: &str) -> Self {
        GroupId(value.to_string())
    }
}

impl AsRef<str> for GroupId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for GroupId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for GroupId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct ArtifactId(String);
impl ArtifactId {
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for ArtifactId {
    fn from(value: String) -> Self {
        ArtifactId(value)
    }
}

impl From<&str> for ArtifactId {
    fn from(value: &str) -> Self {
        ArtifactId(value.to_string())
    }
}

impl AsRef<str> for ArtifactId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for ArtifactId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for ArtifactId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct Version(String);
impl Version {
    pub fn into_string(self) -> String {
        self.0
    }
    pub fn is_snapshot(&self) -> bool {
        self.0.ends_with("-SNAPSHOT")
    }

    pub fn is_meta_version(&self) -> bool {
        self.is_latest() || self.is_release()
    }

    pub fn is_latest(&self) -> bool {
        let lower = self.0.to_lowercase();
        lower == "latest"
    }

    pub fn is_release(&self) -> bool {
        let lower = self.0.to_lowercase();
        lower == "release"
    }
}

impl From<String> for Version {
    fn from(value: String) -> Self {
        Version(value)
    }
}

impl From<&str> for Version {
    fn from(value: &str) -> Self {
        Version(value.to_string())
    }
}

impl AsRef<str> for Version {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for Version {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct Classifier(String);
impl Classifier {
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for Classifier {
    fn from(value: String) -> Self {
        Classifier(value)
    }
}

impl From<&str> for Classifier {
    fn from(value: &str) -> Self {
        Classifier(value.to_string())
    }
}

impl AsRef<str> for Classifier {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for Classifier {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Classifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[derive(Clone)]
pub struct Repository {
    pub url: Url,
    pub snapshots: bool,
    pub releases: bool,
}

impl Repository {
    pub fn maven_central() -> Repository {
        Self::releases(Url::parse("https://repo1.maven.org/maven2/").unwrap())
    }

    fn new(url: Url, snapshots: bool, releases: bool) -> Repository {
        let new_base = if url.path().ends_with("/") {
            let mut new_base = url.clone();
            new_base.set_path(url.path().strip_suffix("/").unwrap());
            new_base
        } else {
            url
        };
        Repository {
            url: new_base,
            snapshots,
            releases,
        }
    }

    pub fn both(url: Url) -> Repository {
        Self::new(url, true, true)
    }

    pub fn releases(url: Url) -> Repository {
        Self::new(url, false, true)
    }
    pub fn snapshots(url: Url) -> Repository {
        Self::new(url, true, false)
    }
}
