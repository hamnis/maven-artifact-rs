use crate::artifact::{Artifact, ParseArtifactError, PartialArtifact, ResolvedArtifact};
use crate::metadata::VersionedMetadata;
use crate::{Repository, Version, metadata};
use reqwest::{Client, Response};
use std::fs::File;
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("Failed to parse url {0}")]
    UrlError(#[from] url::ParseError),
    #[error("Parse artifact {0}")]
    Parse(#[from] ParseArtifactError),
    #[error("Error using reqwest {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("XML decoder error: {0}")]
    XMLDecodeError(#[from] metadata::MetadataError),
    #[error("IO operation failed, {0}")]
    IO(#[from] std::io::Error),
    #[error("Http error, url={url}, status={status}")]
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

    async fn metadata0(&self, path: String) -> Result<VersionedMetadata, ResolveError> {
        let metadata_path = format!("{}/{}/maven-metadata.xml", self.repository.url.path(), path);
        let url = self.repository.url.join(&metadata_path)?;
        let response = self.client.get(url.clone()).send().await?;
        if response.status().is_success() {
            let bytes = response.bytes().await?;
            let c = Cursor::new(bytes);
            let versioned: VersionedMetadata = VersionedMetadata::parse(c)?;
            Ok(versioned)
        } else {
            Err(ResolveError::GenericHttpError {
                url: url.clone(),
                status: response.status().as_u16(),
            })
        }
    }

    pub async fn download(&self, artifact: Artifact, path: &Path) -> Result<PathBuf, ResolveError> {
        if artifact.is_snapshot() {
            if self.repository.snapshots {
                let meta = self.metadata0(artifact.path()).await?;
                let versioning = meta.versioning;
                let snapshot = versioning.snapshot.unwrap();
                let meta_version =
                    Version::from(format!("{}-{}", snapshot.timestamp, snapshot.buildNumber));
                let versions = versioning.snapshotVersions.unwrap_or(vec![]);
                let found = versions.iter().find_map(move |x| {
                    if x.value.ends_with(meta_version.as_ref()) {
                        Some(x.value.clone())
                    } else {
                        None
                    }
                });

                let resolved = ResolvedArtifact {
                    artifact: artifact.clone(),
                    resolved_version: found.unwrap_or(artifact.version.clone()),
                };
                self.download0(resolved, path).await
            } else {
                Err(ResolveError::Message(String::from(
                    "You may not resolve snapshots from a non-snapshot repository",
                )))
            }
        } else if artifact.version.is_meta_version() {
            let meta = self.metadata(artifact.clone().into()).await?;
            let versioning = meta.versioning;
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
        let url = artifact.uri(self.repository)?;
        eprintln!("{}", url);
        let mut response = self.client.get(url.clone()).send().await?;
        let path = dir.join(artifact.artifact.file_name());

        #[cfg(feature = "progressbar")]
        {
            use indicatif::{ProgressBar, ProgressStyle};

            let pb = ProgressBar::no_length();
            match response.content_length() {
                Some(length) => pb.set_length(length),
                None => (),
            };
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
                )
                .unwrap()
                .progress_chars("#>-"),
            );
            let mut file = BufWriter::new(pb.wrap_write(File::create(&path)?));
            Self::write(&mut response, &mut file).await?;
        }
        #[cfg(not(feature = "progressbar"))]
        {
            let mut file = BufWriter::new(File::create(&path)?);
            Self::write(&mut response, &mut file).await?;
        }

        Ok(path)
    }

    async fn write<W: Write>(response: &mut Response, file: &mut W) -> Result<(), ResolveError> {
        // Stream the response body and write it to the file chunk by chunk
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk)?;
        }
        Ok(())
    }
}
