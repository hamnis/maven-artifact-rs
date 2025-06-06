use crate::artifact::{Artifact, ParseArtifactError, PartialArtifact, ResolvedArtifact};
use crate::metadata::VersionedMetadata;
use crate::{Repository, Version};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use reqwest::Client;
use std::path::{Path, PathBuf};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("Failed to parse url")]
    UrlError(#[from] url::ParseError),
    #[error("Parse artifact")]
    Parse(#[from] ParseArtifactError),
    #[error("Error using reqwest")]
    Reqwest(#[from] reqwest::Error),
    #[error("XML decoder error")]
    XMLDecodeError(#[from] serde_xml_rs::Error),
    #[error("IO operation failed")]
    IO(#[from] std::io::Error),
    #[error("Http error")]
    GenericHttpError { url: Url, status: u16 },
    #[error("Resolve error {0}")]
    Message(String),
}

pub struct Resolver<'a> {
    client: &'a Client,
    repository: &'a Repository,
}

impl Resolver<'_> {
    pub fn new<'a>(client: &'a Client, repository: &'a Repository) -> Resolver<'a> {
        Resolver { client, repository }
    }

    pub async fn metadata(
        &self,
        artifact: PartialArtifact,
    ) -> Result<VersionedMetadata, ResolveError> {
        self.metadata0(artifact.path()).await
    }

    async fn get(
        &self,
        url: Url,
    ) -> Result<impl Stream<Item = reqwest::Result<Bytes>>, ResolveError> {
        let response = self.client.get(url.clone()).send().await?;
        if response.status().is_success() {
            let bytes = response.bytes_stream();
            Ok(bytes)
        } else {
            Err(ResolveError::GenericHttpError {
                url: url.clone(),
                status: response.status().as_u16(),
            })
        }
    }

    async fn metadata0(&self, path: String) -> Result<VersionedMetadata, ResolveError> {
        let metadata_path = format!("{}/metadata-xml", path);
        let mut stream = self.get(self.repository.url.join(&metadata_path)?).await?;
        let mut tmp_file = tokio::fs::File::from(tempfile::tempfile()?);
        while let Some(item) = stream.next().await {
            tokio::io::copy(&mut item?.as_ref(), &mut tmp_file).await?;
        }
        tmp_file.sync_all().await?;
        let std = tmp_file.into_std().await;
        let versioned: VersionedMetadata = serde_xml_rs::from_reader(std)?;
        Ok(versioned)
    }

    pub async fn download(&self, artifact: Artifact, path: &Path) -> Result<PathBuf, ResolveError> {
        if artifact.is_snapshot() {
            if self.repository.snapshots {
                let meta = self.metadata0(artifact.path()).await?;
                let versioning = meta.versioning.unwrap();
                let snapshot = versioning.snapshot.unwrap();
                let meta_version =
                    Version::from(format!("{}-{}", snapshot.timestamp, snapshot.buildNumber));
                //let versions = versioning.snapshotVersions.unwrap_or(vec![]);
                let resolved = ResolvedArtifact {
                    artifact: artifact.clone(),
                    resolved_version: meta_version,
                };
                self.download0(resolved, path).await
            } else {
                Err(ResolveError::Message(String::from(
                    "You may not resolve snapshots from a non-snapshot repository",
                )))
            }
        } else if artifact.version.is_meta_version() {
            let meta = self.metadata(artifact.clone().into()).await?;
            let versioning = meta.versioning.unwrap();
            let maybe_resolved = if artifact.version.is_release() {
                versioning.release
            } else {
                versioning.latest
            };
            match maybe_resolved {
                None => Err(ResolveError::Message(format!(
                    "Failed to download artifact {}",
                    artifact
                ))),
                Some(resolved) => {
                    self.download0(
                        ResolvedArtifact {
                            artifact: artifact.clone(),
                            resolved_version: resolved,
                        },
                        path,
                    )
                    .await
                }
            }
        } else {
            self.download0(
                ResolvedArtifact {
                    artifact: artifact.clone(),
                    resolved_version: artifact.version.clone(),
                },
                path,
            )
            .await
        }
    }
    async fn download0(
        &self,
        artifact: ResolvedArtifact,
        dir: &Path,
    ) -> Result<PathBuf, ResolveError> {
        let mut stream = self.get(artifact.uri(self.repository)?).await?;
        let path = dir.join(artifact.artifact.file_name());
        let mut tmp_file = tokio::fs::File::from(tempfile::tempfile()?);
        while let Some(item) = stream.next().await {
            tokio::io::copy(&mut item?.as_ref(), &mut tmp_file).await?;
        }
        tmp_file.sync_all().await?;

        Ok(path)
    }
}
