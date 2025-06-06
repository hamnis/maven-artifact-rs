use crate::*;
use std::fmt::{Display, Formatter};
use url::Url;

#[derive(Debug, Clone, Error)]
#[error("{0}")]
pub struct ParseArtifactError(String);

pub struct PartialArtifact {
    pub group_id: GroupId,
    pub artifact_id: ArtifactId,
}

impl PartialArtifact {
    pub fn new(group_id: GroupId, artifact_id: ArtifactId) -> PartialArtifact {
        PartialArtifact {
            group_id,
            artifact_id,
        }
    }

    pub fn into_artifact(self, version: Version) -> Artifact {
        Artifact::new(self.group_id.clone(), self.artifact_id.clone(), version)
    }

    pub fn path(&self) -> String {
        format!("{}/{}", self.group_id.path_string(), self.artifact_id)
    }
}

impl Display for PartialArtifact {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.group_id, self.artifact_id)
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Debug)]
pub struct Artifact {
    pub group_id: GroupId,
    pub artifact_id: ArtifactId,
    pub version: Version,
    pub extension: Option<String>,
    pub classifier: Option<Classifier>,
}

impl Artifact {
    pub fn new(group_id: GroupId, artifact_id: ArtifactId, version: Version) -> Artifact {
        Artifact {
            group_id,
            artifact_id,
            version,
            extension: None,
            classifier: None,
        }
    }

    pub fn with_version(&self, version: Version) -> Artifact {
        let mut cloned = self.clone();
        cloned.version = version;
        cloned
    }

    pub fn with_classifier(&self, classifier: Classifier) -> Artifact {
        let mut cloned = self.clone();
        cloned.classifier = Some(classifier);
        cloned
    }

    pub fn with_extension(&self, extension: String) -> Artifact {
        let mut cloned = self.clone();
        cloned.extension = Some(extension);
        cloned
    }

    pub fn without_extension(&self) -> Artifact {
        let mut cloned = self.clone();
        cloned.extension = None;
        cloned
    }

    pub fn is_snapshot(&self) -> bool {
        self.version.is_snapshot()
    }

    pub fn path(&self) -> String {
        let base = format!("{}/{}", self.group_id.path_string(), self.artifact_id);
        format!("{}/{}", base, self.version)
    }

    pub fn file_name(&self) -> String {
        format!(
            "{}.{}",
            self.artifact_id,
            self.extension.as_deref().unwrap_or("jar")
        )
    }

    pub fn parse(input: String) -> Result<Artifact, ParseArtifactError> {
        let parts: Vec<_> = input.split(":").collect();
        if parts.len() >= 3 {
            let (ga, rest) = parts.split_at(2);
            match (&ga[..], &rest[..]) {
                ([g, a], [v]) => Ok(Artifact {
                    group_id: GroupId(g.to_string()),
                    artifact_id: ArtifactId(a.to_string()),
                    version: Version(v.to_string()),
                    extension: None,
                    classifier: None,
                }),
                ([g, a], [e, v]) => Ok(Artifact {
                    group_id: GroupId(g.to_string()),
                    artifact_id: ArtifactId(a.to_string()),
                    version: Version(v.to_string()),
                    extension: Some(e.to_string()),
                    classifier: None,
                }),
                ([g, a], [e, c, v]) => Ok(Artifact {
                    group_id: GroupId(g.to_string()),
                    artifact_id: ArtifactId(a.to_string()),
                    version: Version(v.to_string()),
                    extension: Some(e.to_string()),
                    classifier: Some(Classifier(c.to_string())),
                }),
                _ => Err(ParseArtifactError(String::from("Unable to parse artifact"))),
            }
        } else {
            Err(ParseArtifactError(format!(
                "Incorrect number of parts. Expected as least 3, but was {}",
                parts.len()
            )))
        }
    }
}

impl From<Artifact> for PartialArtifact {
    fn from(value: Artifact) -> Self {
        PartialArtifact::new(value.group_id, value.artifact_id)
    }
}

impl Display for Artifact {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut gav = format!("{}:{}", self.group_id, self.artifact_id);

        match (self.extension.clone(), self.classifier.clone()) {
            (Some(e), Some(c)) => {
                let ec = format!(":{}:{}", e, c);
                gav += ec.as_str()
            }
            (None, Some(c)) => {
                let ec = format!(":{}:{}", "jar", c);
                gav += ec.as_str()
            }
            (Some(e), None) if e != "jar" => {
                let ec = format!(":{}", e);
                gav += ec.as_str()
            }
            _ => (),
        }
        gav += format!(":{}", self.version).as_str();
        f.write_str(gav.as_str())
    }
}

pub struct ResolvedArtifact {
    pub artifact: Artifact,
    pub resolved_version: Version,
}

impl ResolvedArtifact {
    pub fn path(&self) -> String {
        let base = format!(
            "{}/{}",
            self.artifact.group_id.path_string(),
            self.artifact.artifact_id
        );
        let version = if self.artifact.is_snapshot() {
            &self.artifact.version
        } else {
            &self.resolved_version
        };

        format!("{}/{}", base, version)
    }

    pub fn uri(&self, repository: &Repository) -> Result<Url, url::ParseError> {
        let mut current_path = format!(
            "{}/{}/{}-{}",
            repository.url.path(),
            self.path(),
            self.artifact.artifact_id,
            self.resolved_version
        );
        if let Some(c) = self.artifact.classifier.clone() {
            current_path += format!("-{}", c).as_str()
        }
        current_path +=
            format!(".{}", self.artifact.extension.as_deref().unwrap_or("jar")).as_str();
        repository.url.join(current_path.as_str())
    }
}

impl From<ResolvedArtifact> for Artifact {
    fn from(value: ResolvedArtifact) -> Self {
        value.artifact.clone().with_version(value.resolved_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gav() {
        let result = Artifact::parse(String::from("g:a:v")).unwrap();
        assert_eq!(
            &result,
            &Artifact::new(
                GroupId::from("g"),
                ArtifactId::from("a"),
                Version::from("v")
            )
        );
        assert_eq!(&result.to_string(), "g:a:v")
    }
    #[test]
    fn parse_full_gav() {
        let input = "groupId:artifactId:packaging:classifier:version";
        let result = Artifact::parse(String::from(input)).unwrap();
        assert_eq!(
            result,
            Artifact {
                group_id: GroupId::from("groupId"),
                artifact_id: ArtifactId::from("artifactId"),
                version: Version::from("version"),
                classifier: Some(Classifier::from("classifier")),
                extension: Some(String::from("packaging"))
            }
        );
        assert_eq!(result.to_string(), String::from(input))
    }

    #[test]
    fn parse_missing_classifier() {
        let input = "groupId:artifactId:packaging:version";
        let result = Artifact::parse(String::from(input)).unwrap();
        assert_eq!(
            result,
            Artifact {
                group_id: GroupId::from("groupId"),
                artifact_id: ArtifactId::from("artifactId"),
                version: Version::from("version"),
                classifier: None,
                extension: Some(String::from("packaging"))
            }
        );
        assert_eq!(result.to_string(), String::from(input))
    }

    #[test]
    fn resolved_uri() {
        let a = Artifact::new(
            GroupId::from("com.example"),
            ArtifactId::from("artifact"),
            Version::from("1.0.0"),
        );
        let resolved = ResolvedArtifact {
            artifact: a,
            resolved_version: Version::from("1.0.0"),
        };

        let base = Repository::maven_central();
        let parsed = resolved.uri(&base).unwrap();
        let expected = base
            .clone()
            .url
            .join("/maven2/com/example/artifact/1.0.0/artifact-1.0.0.jar")
            .unwrap();
        assert_eq!(parsed, expected)
    }
}
