use crate::*;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use url::Url;

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

    pub fn parse(input: String) -> Result<Artifact, Error> {
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
                _ => Err(Error::ParseArtifactError(String::from(
                    "Unable to parse artifact",
                ))),
            }
        } else {
            Err(Error::ParseArtifactError(format!(
                "Incorrect number of parts. Expected as least 3, but was {}",
                parts.len()
            )))
        }
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
    pub fn path(&self, with_version: bool) -> String {
        let base =
            self.artifact.group_id.replace(".", "/") + "/" + self.artifact.artifact_id.deref();

        if with_version {
            format!("{}/{}", base, self.resolved_version)
        } else {
            base
        }
    }

    pub fn uri(&self, base: Url) -> Result<Url, Error> {
        let newBase = if (base.path().ends_with("/")) {
            let mut newBase = base.clone();
            newBase.set_path(base.path().strip_suffix("/").unwrap());
            newBase
        } else {
            base
        };

        let mut current_path = format!(
            "{}/{}-{}",
            self.path(true),
            self.artifact.artifact_id,
            self.resolved_version
        );
        if let Some(c) = self.artifact.classifier.clone() {
            current_path += format!("-{}", c).as_str()
        }
        current_path +=
            format!(".{}", self.artifact.extension.as_deref().unwrap_or("jar")).as_str();
        match newBase.join(current_path.as_str()) {
            Err(p) => Err(Error::UrlError(p)),
            Ok(u) => Ok(u),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gav() {
        let result = Artifact::parse(String::from("g:a:v"));
        assert_eq!(
            result,
            Ok(Artifact::new(
                GroupId::from("g"),
                ArtifactId::from("a"),
                Version::from("v")
            ))
        );
        assert_eq!(result.map(|a| a.to_string()), Ok(String::from("g:a:v")))
    }
    #[test]
    fn parse_full_gav() {
        let input = "groupId:artifactId:packaging:classifier:version";
        let result = Artifact::parse(String::from(input));
        assert_eq!(
            result,
            Ok(Artifact {
                group_id: GroupId::from("groupId"),
                artifact_id: ArtifactId::from("artifactId"),
                version: Version::from("version"),
                classifier: Some(Classifier::from("classifier")),
                extension: Some(String::from("packaging"))
            })
        );
        assert_eq!(result.map(|a| a.to_string()), Ok(String::from(input)))
    }

    #[test]
    fn parse_missing_classifier() {
        let input = "groupId:artifactId:packaging:version";
        let result = Artifact::parse(String::from(input));
        assert_eq!(
            result,
            Ok(Artifact {
                group_id: GroupId::from("groupId"),
                artifact_id: ArtifactId::from("artifactId"),
                version: Version::from("version"),
                classifier: None,
                extension: Some(String::from("packaging"))
            })
        );
        assert_eq!(result.map(|a| a.to_string()), Ok(String::from(input)))
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

        let base = Url::parse("https://repo1.maven.org/maven2/").unwrap();
        let parsed = resolved.uri(base.clone());
        let expected = base
            .clone()
            .join("/com/example/artifact/1.0.0/artifact-1.0.0.jar")
            .unwrap();
        assert_eq!(parsed, Ok(expected))
    }
}
