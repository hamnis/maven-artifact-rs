use std::collections::HashMap;
use crate::artifact::{Artifact, ParseArtifactError, PartialArtifact, ResolvedArtifact};
use crate::metadata::VersionedMetadata;
use crate::project::{Dependency, Project, ProjectReference};
use crate::{Repository, Version};
use bytes::Bytes;
use reqwest::{Client, Response};
use std::fs::File;
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};
use futures::future::join_all;
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
    #[error("XML Metadata decoder error: {0}")]
    XMLMetadataDecodeError(#[from] crate::metadata::MetadataError),
    #[error("XML Project decoder error: {0}")]
    XMLProjectDecodeError(#[from] crate::pom::PomParserError),
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
        self.maven_metadata(artifact.path()).await
    }

    pub async fn project_metadata(
        &self,
        artifact: ProjectReference,
    ) -> Result<Project, ResolveError> {
        self.pom_metadata(artifact).await
    }

    fn maven_metadata(
        &self,
        path: String,
    ) -> impl Future<Output = Result<VersionedMetadata, ResolveError>> {
        self.metadata0(path, "maven-metadata.xml", VersionedMetadata::parse)
    }

    fn pom_metadata(
        &self,
        artifact: ProjectReference,
    ) -> impl Future<Output = Result<Project, ResolveError>> {
        self.metadata0(artifact.path(), "pom.xml", Project::parse)
    }

    async fn metadata0<A, E, FN>(
        &self,
        path: String,
        file: &str,
        parse_from: FN,
    ) -> Result<A, ResolveError>
    where
        FN: Fn(Cursor<Bytes>) -> Result<A, E>,
        ResolveError: From<E>,
    {
        let metadata_path = format!("{}/{}/{file}", self.repository.url.path(), path);
        let url = self.repository.url.join(&metadata_path)?;
        let response = self.client.get(url.clone()).send().await?;
        if response.status().is_success() {
            let bytes = response.bytes().await?;
            let c = Cursor::new(bytes);
            let versioned = parse_from(c)?;
            Ok(versioned)
        } else {
            Err(ResolveError::GenericHttpError {
                url: url.clone(),
                status: response.status().as_u16(),
            })
        }
    }

    pub async fn download(
        &self,
        artifact: &Artifact,
        path: &Path,
    ) -> Result<PathBuf, ResolveError> {
        let version = artifact
            .version
            .clone()
            .ok_or(ResolveError::Message(String::from("No version set")))?;
        if artifact.is_snapshot() {
            if self.repository.snapshots {
                let meta = self.maven_metadata(artifact.path()).await?;
                let versioning = meta.versioning;
                let snapshot = versioning.snapshot.unwrap();
                let meta_version =
                    Version::from(format!("{}-{}", snapshot.timestamp, snapshot.buildNumber));
                let versions = versioning.snapshot_versions.unwrap_or(vec![]);
                let found = versions.iter().find_map(move |x| {
                    if x.value.ends_with(meta_version.as_ref()) {
                        Some(x.value.clone())
                    } else {
                        None
                    }
                });

                let resolved = ResolvedArtifact {
                    artifact: artifact.clone(),
                    resolved_version: found.or(artifact.version.clone()).unwrap(),
                };
                self.download0(resolved, path).await
            } else {
                Err(ResolveError::Message(String::from(
                    "You may not resolve snapshots from a non-snapshot repository",
                )))
            }
        } else if version.is_meta_version() {
            let meta = self.maven_metadata(artifact.path()).await?;
            let versioning = meta.versioning;
            let maybe_resolved = if version.is_release() {
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
                    resolved_version: version.clone(),
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
        eprintln!("Downloading {}", url);
        let mut response = self.client.get(url.clone()).send().await?;
        let path = dir.join(artifact.artifact.file_name());

        #[cfg(feature = "progressbar")]
        {
            use indicatif::{ProgressBar, ProgressStyle};

            let pb = ProgressBar::no_length();
            if let Some(length) = response.content_length() {
                pb.set_length(length)
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


    async fn get_parent(&self, project: &Project) -> Option<Project> {
        if let Some(p) = &project.parent {
            let next = self
                .project_metadata(ProjectReference::from(p))
                .await
                .ok()?;
            Some(next)
        } else {
            None
        }
    }

    async fn get_parents(&self, project: &Project) -> Vec<Project> {
        let mut parents = vec![];
        let mut maybe_parent = self.get_parent(project).await;
        while let Some(p) = maybe_parent {
            parents.push(p.clone());
            maybe_parent = self.get_parent(&p).await
        }
        parents
    }

    pub async fn collect_dependencies(
        &self,
        artifact: &Artifact,
    ) -> Result<Vec<Artifact>, ResolveError> {
        let mut vec = vec![];

        let project = self
            .project_metadata(ProjectReference::from(artifact))
            .await?;
        let parents = self.get_parents(&project).await;
        let boms = self.get_boms_from_all(&project, &parents).await?;
        let mut props = parents.iter().rfold(HashMap::new(), |mut p, item| {
            p.extend(item.properties.clone());
            p
        });
        props.extend(project.properties);

        for dep in project.dependencies {
            let dependency = dep.resolve_properties(&props);
            if dependency.artifact.version.is_some() {
                vec.push(dependency.artifact.clone())
            } else {
                if let Some(resolved) = boms.get(&dependency.mngt_key()) {
                    vec.push(resolved.artifact.clone())
                }
            }
        }

        Ok(vec)
    }

    async fn get_boms_from_all(
        &self,
        project: &Project,
        parents: &[Project],
    ) -> Result<HashMap<String, Dependency>, ResolveError> {
        let mut dependencies: HashMap<String, Dependency> = HashMap::new();
        let resolved: Result<Vec<Vec<Dependency>>, ResolveError> = join_all(
            parents
                .into_iter()
                .map(|x| self.get_bill_of_materials(x)),
        )
            .await
            .into_iter()
            .collect();

        for parent_boms in resolved? {
            dependencies.extend(Dependency::mapped(&parent_boms))
        }

        let boms = self.get_bill_of_materials(project).await?;
        dependencies.extend(Dependency::mapped(&boms));

        Ok(dependencies)
    }

    async fn get_bill_of_materials(
        &self,
        project: &Project,
    ) -> Result<Vec<Dependency>, ResolveError> {
        let mut dependencies: Vec<Dependency> = vec![];
        let bill_of_materials_deps: Vec<&Dependency> = project
            .dependency_management
            .dependencies
            .iter()
            .filter(|d| d.is_scope("import") && d.artifact.ext_or_jar() == "pom")
            .collect();

        let all: Result<Vec<Project>, ResolveError> = join_all(
            bill_of_materials_deps
                .into_iter()
                .map(|d| self.project_metadata(ProjectReference::from(&d.artifact))),
        )
            .await
            .into_iter()
            .collect();

        for p in all? {
            dependencies.extend(p.resolve_properties_this().dependencies);
        }

        Ok(dependencies)
    }
}
