use crate::artifact::{Artifact, PartialArtifact, ResolvedArtifact};
use crate::metadata::VersionedMetadata;
use crate::{MavenError, Repository};
use reqwest::{Client, StatusCode};
use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};

struct Resolver<'a> {
    client: &'a Client,
    repository: &'a Repository,
}

impl Resolver<'_> {
    fn new<'a>(client: &'a Client, repository: &'a Repository) -> Resolver<'a> {
        Resolver { client, repository }
    }

    async fn metadata(&self, artifact: PartialArtifact) -> Result<VersionedMetadata, MavenError> {
        self.metadata0(artifact.path(), artifact.to_string()).await
    }

    async fn metadata0(
        &self,
        path: String,
        rendered_artifact: String,
    ) -> Result<VersionedMetadata, MavenError> {
        let metadata_path = format!("{}/metadata-xml", path);
        let builder = self.client.get(self.repository.url.join(&metadata_path)?);
        let response = builder.send().await?;
        if response.status().is_success() {
            let bytes = response.bytes().await?;
            let cursor = Cursor::new(bytes);
            let versioned: VersionedMetadata = serde_xml_rs::from_reader(cursor)?;
            Ok(versioned)
        } else if response.status() == StatusCode::NOT_FOUND {
            Err(MavenError::NotFoundError(metadata_path))
        } else {
            Err(MavenError::ResolveMessageError(format!(
                "Failed to resolve metadata, {}",
                rendered_artifact
            )))
        }
    }

    async fn download(&self, artifact: Artifact, path: &Path) -> Result<PathBuf, MavenError> {
        if artifact.is_snapshot() {
            let meta = self
                .metadata0(artifact.path(), artifact.to_string())
                .await?;
            let versioning = meta.versioning.unwrap();
            
            todo!()
        } else if artifact.version.is_meta_version() {
            let meta = self.metadata(artifact.clone().into()).await?;
            let versioning = meta.versioning.unwrap();
            let maybe_resolved = if artifact.version.is_release() {
                versioning.release
            } else {
                versioning.latest
            };
            match maybe_resolved {
                None => Err(MavenError::ResolveMessageError(format!(
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
    ) -> Result<PathBuf, MavenError> {
        let builder = self.client.get(artifact.uri(self.repository)?);
        let response = builder.send().await?;
        if response.status().is_success() {
            let path = dir.join(artifact.artifact.file_name());
            let mut file = File::create(&path)?;
            let bytes = response.bytes().await?;
            let mut cursor = Cursor::new(bytes);
            std::io::copy(&mut cursor, &mut file)?;
            Ok(path)
        } else {
            let message_artifact: Artifact = artifact.into();
            Err(MavenError::ResolveMessageError(format!(
                "Failed to download artifact, {}",
                message_artifact
            )))
        }
    }
}
