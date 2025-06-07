use crate::metadata::MetadataError::Unexpected;
pub use crate::{ArtifactId, Classifier, GroupId, Version};
use std::io::{BufReader, Cursor, Read, Seek};
use std::num::ParseIntError;
use thiserror::Error;
use xml::EventReader;
use xml::reader::XmlEvent;

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("{0} IO error while parsing")]
    IO(#[from] std::io::Error),
    #[error("{0} XML error while parsing")]
    XML(#[from] xml::reader::Error),
    #[error("{0} Failed to parse integer")]
    IntParse(#[from] ParseIntError),
    #[error("{0} Unexpected XML error while parsing")]
    Unexpected(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VersionedMetadata {
    pub group_id: GroupId,
    pub artifact_id: ArtifactId,
    pub versioning: Versioning,
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Versioning {
    pub latest: Option<Version>,
    pub release: Option<Version>,
    pub versions: Option<Vec<Version>>,
    pub last_updated: Option<String>,
    pub snapshot: Option<Snapshot>,
    pub snapshot_versions: Option<Vec<SnapshotVersion>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub timestamp: String,
    pub buildNumber: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotVersion {
    pub classifier: Option<Classifier>,
    pub extension: Option<String>,
    pub value: Version,
    pub updated: String,
}

impl SnapshotVersion {
    pub fn new(
        value: Version,
        updated: String,
        classifier: Option<Classifier>,
        extension: Option<String>,
    ) -> SnapshotVersion {
        SnapshotVersion {
            value,
            updated,
            classifier,
            extension,
        }
    }
}

impl VersionedMetadata {
    pub fn from_str(input: &str) -> Result<VersionedMetadata, MetadataError> {
        Self::parse(Cursor::new(input))
    }

    pub fn parse<R: Read + Seek>(input: R) -> Result<VersionedMetadata, MetadataError> {
        let buffer = BufReader::new(input);
        let mut parser = EventReader::new(buffer);
        let mut group_id: Option<GroupId> = None;
        let mut artifact_id: Option<ArtifactId> = None;
        let mut versioning: Option<Versioning> = None;

        loop {
            let event = &parser.next()?;
            match event {
                XmlEvent::EndDocument => match (&group_id, &artifact_id, &versioning) {
                    (Some(g), Some(a), Some(v)) => {
                        break Ok(VersionedMetadata {
                            group_id: g.clone(),
                            artifact_id: a.clone(),
                            versioning: v.clone(),
                        });
                    }
                    (None, _, _) => {
                        break Err(Unexpected(String::from("Missing groupId")));
                    }
                    (_, None, _) => {
                        break Err(Unexpected(String::from("Missing artifact_id")));
                    }
                    (_, _, None) => {
                        break Err(Unexpected(String::from("Missing versioning")));
                    }
                },
                XmlEvent::StartElement { name, .. } if name.local_name == "groupId" => {
                    let id = Self::string_element(&mut parser)?;
                    group_id = Some(GroupId::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "artifactId" => {
                    let id = Self::string_element(&mut parser)?;
                    artifact_id = Some(ArtifactId::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "versioning" => {
                    let v = Self::parse_versionining(&mut parser)?;
                    versioning = Some(v);
                }
                _ => continue,
            }
        }
    }

    fn parse_versionining<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<Versioning, MetadataError> {
        let mut parsed: Versioning = Versioning::default();
        let mut versions: Vec<Version> = Vec::new();
        let mut snapshots: Vec<SnapshotVersion> = Vec::new();
        loop {
            let event = &parser.next()?;
            match event {
                XmlEvent::EndElement { name, .. } if name.local_name == "versioning" => {
                    break Ok(parsed.clone());
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "latest" => {
                    let version = Self::string_element(parser)?;
                    parsed.latest = Some(Version::from(version));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "release" => {
                    let version = Self::string_element(parser)?;
                    parsed.release = Some(Version::from(version));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "version" => {
                    let version = Self::string_element(parser)?;
                    versions.push(Version::from(version));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "lastUpdated" => {
                    let updated = Self::string_element(parser)?;
                    parsed.last_updated = Some(updated);
                }
                XmlEvent::EndElement { name, .. } if name.local_name == "versions" => {
                    parsed.versions = Some(versions.clone());
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "snapshot" => {
                    let snapshot = Self::parse_snapshot(parser)?;
                    parsed.snapshot = Some(snapshot);
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "snapshotVersion" => {
                    let version = Self::parse_snapshot_version(parser)?;
                    snapshots.push(version);
                }
                XmlEvent::EndElement { name, .. } if name.local_name == "snapshotVersions" => {
                    parsed.snapshot_versions = Some(snapshots.clone());
                }
                _ => continue,
            }
        }
    }

    fn parse_snapshot<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<Snapshot, MetadataError> {
        let mut timestamp: Option<String> = None;
        let mut build_number: Option<i32> = None;
        loop {
            match parser.next()? {
                XmlEvent::EndElement { name, .. } if name.local_name == "snapshot" => {
                    let result = match (timestamp, build_number) {
                        (Some(t), Some(b)) => Ok(Snapshot {
                            timestamp: t,
                            buildNumber: b,
                        }),
                        (None, _) => Err(Unexpected(String::from("Timestamp is missing"))),
                        (_, None) => Err(Unexpected(String::from("buildNumber is missing"))),
                    };
                    break result;
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "timestamp" => {
                    let updated = Self::string_element(parser)?;
                    timestamp = Some(updated);
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "buildNumber" => {
                    let updated = Self::string_element(parser)?;
                    let parsed = updated.parse::<i32>()?;
                    build_number = Some(parsed);
                }
                _ => continue,
            }
        }
    }
    fn parse_snapshot_version<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<SnapshotVersion, MetadataError> {
        let mut value: Option<Version> = None;
        let mut updated: Option<String> = None;
        let mut extension: Option<String> = None;
        let mut classifier: Option<Classifier> = None;
        loop {
            match parser.next()? {
                XmlEvent::EndElement { name, .. } if name.local_name == "snapshotVersion" => {
                    let result = match (value, updated) {
                        (Some(v), Some(up)) => Ok(SnapshotVersion {
                            extension,
                            classifier,
                            value: v,
                            updated: up,
                        }),
                        (None, _) => Err(Unexpected(String::from("Timestamp is missing"))),
                        (_, None) => Err(Unexpected(String::from("buildNumber is missing"))),
                    };
                    break result;
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "value" => {
                    let updated = Self::string_element(parser)?;
                    value = Some(Version::from(updated));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "updated" => {
                    let up = Self::string_element(parser)?;
                    updated = Some(up);
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "extension" => {
                    let updated = Self::string_element(parser)?;
                    extension = Some(updated);
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "classifier" => {
                    let updated = Self::string_element(parser)?;
                    classifier = Some(Classifier::from(updated));
                }
                _ => continue,
            }
        }
    }

    fn string_element<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<String, MetadataError> {
        let out = match &parser.next()? {
            XmlEvent::Characters(chars) => Ok(chars.to_owned()),
            e => Err(Unexpected(format!("{:?}", e))),
        }?;
        parser.next()?;
        Ok(out)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_simple() {
        let meta = r##"<?xml version="1.0" encoding="UTF-8"?><metadata><groupId>com.example</groupId><artifactId>example-cli</artifactId><versioning><latest>3.0.0</latest><release>3.0.0</release><versions><version>3.0.0</version></versions><lastUpdated>20250427133131</lastUpdated></versioning></metadata>"##;

        let metadata: VersionedMetadata = VersionedMetadata::from_str(meta).unwrap();
        assert_eq!(
            metadata,
            VersionedMetadata {
                group_id: GroupId::from("com.example"),
                artifact_id: ArtifactId::from("example-cli"),
                versioning: Versioning {
                    latest: Some(Version::from("3.0.0")),
                    release: Some(Version::from("3.0.0")),
                    versions: Some(vec![Version::from("3.0.0")]),
                    last_updated: Some(String::from("20250427133131")),
                    snapshot: None,
                    snapshot_versions: None
                }
            }
        )
    }

    #[test]
    fn parse_more_complicated() {
        let input = std::fs::read_to_string(
            "test-files/metadata/org/openapitools/openapi-generator-cli/maven-metadata.xml",
        )
        .unwrap();
        let metadata: VersionedMetadata = VersionedMetadata::from_str(&input).unwrap();
        assert_eq!(metadata.group_id, GroupId::from("org.openapitools"));
        let versioning = metadata.versioning;
        assert!(versioning.snapshot_versions.is_none());
        let versions = versioning.versions.unwrap();
        assert_eq!(versions.first().unwrap(), &Version::from("3.0.0"));
        assert_eq!(versions.last(), versioning.release.as_ref());
    }

    #[test]
    fn parse_snapshot() {
        let input =
            std::fs::read_to_string("test-files/metadata/org/pac4j/pac4j-http/maven-metadata.xml")
                .unwrap();
        let metadata: VersionedMetadata = VersionedMetadata::from_str(&input).unwrap();
        assert_eq!(metadata.group_id, GroupId::from("org.pac4j"));
        assert_eq!(metadata.artifact_id, ArtifactId::from("pac4j-http"));
        let versioning = metadata.versioning;
        assert!(versioning.snapshot_versions.is_none());
        let versions = versioning.versions.unwrap();
        assert_eq!(versions.first().unwrap(), &Version::from("6.1.4-SNAPSHOT"));
    }

    #[test]
    fn parse_snapshot_version() {
        let input = std::fs::read_to_string(
            "test-files/metadata/org/pac4j/pac4j-http/6.1.4-SNAPSHOT/maven-metadata.xml",
        )
        .unwrap();
        let metadata: VersionedMetadata = VersionedMetadata::from_str(&input).unwrap();
        fn make(extension: &str, classifier: Option<&str>) -> SnapshotVersion {
            SnapshotVersion::new(
                Version::from("6.1.4-20250607.033109-15"),
                String::from("20250607033109"),
                classifier.map(Classifier::from),
                Some(String::from(extension)),
            )
        }

        let expected = VersionedMetadata {
            group_id: GroupId::from("org.pac4j"),
            artifact_id: ArtifactId::from("pac4j-http"),
            versioning: Versioning {
                last_updated: Some(String::from("20250607033109")),
                snapshot: Some(Snapshot {
                    timestamp: String::from("20250607.033109"),
                    buildNumber: 15,
                }),
                snapshot_versions: Some(vec![
                    make("jar", None),
                    make("pom", None),
                    make("jar", Some("javadoc")),
                    make("jar", Some("test-javadoc")),
                    make("jar", Some("tests")),
                    make("jar", Some("sources")),
                    make("jar", Some("test-sources")),
                ]),
                ..Default::default()
            },
        };

        assert_eq!(metadata, expected)
    }
}
