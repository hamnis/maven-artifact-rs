pub use crate::{ArtifactId, Classifier, GroupId, Version};
use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct VersionedMetadata {
    pub groupId: GroupId,
    pub artifactId: ArtifactId,
    pub versioning: Option<Versioning>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Versions {
    version: Vec<Version>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Versioning {
    pub latest: Option<Version>,
    pub release: Option<Version>,
    pub versions: Versions,
    pub lastUpdated: Option<String>,
    pub snapshot: Option<Snapshot>,
    pub snapshotVersions: Option<Vec<SnapshotVersion>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub timestamp: String,
    pub buildNumber: i32,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SnapshotVersion {
    pub classifier: Option<Classifier>,
    pub extension: Option<String>,
    pub version: Version,
    pub updated: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_xml_rs::from_str;

    #[test]
    fn parse_simple() {
        let meta = r##"<?xml version="1.0" encoding="UTF-8"?><metadata><groupId>com.example</groupId><artifactId>example-cli</artifactId><versioning><latest>3.0.0</latest><release>3.0.0</release><versions><version>3.0.0</version></versions><lastUpdated>20250427133131</lastUpdated></versioning></metadata>"##;

        let metadata: VersionedMetadata = from_str(meta).unwrap();
        assert_eq!(
            metadata,
            VersionedMetadata {
                groupId: GroupId::from("com.example"),
                artifactId: ArtifactId::from("example-cli"),
                versioning: Some(Versioning {
                    latest: Some(Version::from("3.0.0")),
                    release: Some(Version::from("3.0.0")),
                    versions: Versions {
                        version: vec![Version::from("3.0.0")]
                    },
                    lastUpdated: Some(String::from("20250427133131")),
                    snapshot: None,
                    snapshotVersions: None
                })
            }
        )
    }

    #[test]
    fn parse_more_complicated() {
        let input = std::fs::read_to_string(
            "test-files/metadata/org/openapitools/openapi-generator-cli/maven-metadata.xml",
        )
        .unwrap();
        let metadata: VersionedMetadata = from_str(&input).unwrap();
        assert_eq!(metadata.groupId, GroupId::from("org.openapitools"));
        assert!(metadata.versioning.is_some());
        let versioning = metadata.versioning.unwrap();
        assert!(versioning.snapshotVersions.is_none());
        assert_eq!(
            versioning.versions.version.first().unwrap(),
            &Version::from("3.0.0")
        );
        assert_eq!(
            versioning.versions.version.last(),
            versioning.release.as_ref()
        );
    }
}
