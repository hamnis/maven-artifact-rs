use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Deref;

mod artifact;
mod metadata;

#[derive(PartialEq, Debug)]
pub enum Error {
    ParseArtifactError(String),
    UrlError(url::ParseError),
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Hash, Debug, Serialize, Deserialize)]
pub struct GroupId(String);

impl GroupId {
    pub fn into_string(self) -> String {
        self.0
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
