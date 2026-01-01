use anyhow::{Context, bail};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use clap::{Parser, Subcommand};
use futures::future::join_all;
use maven_artifact::Repository;
use maven_artifact::artifact::{Artifact, PartialArtifact};
use maven_artifact::project::{Dependency, Project, ProjectReference};
use maven_artifact::resolver::{ResolveError, Resolver};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::{Client, ClientBuilder};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use url::Url;

// Name your user agent after your app?
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Clone)]
enum Select {
    Latest,
    Release,
    Versions,
}

impl FromStr for Select {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "latest" => Ok(Self::Latest),
            "release" => Ok(Self::Release),
            "versions" => Ok(Self::Versions),
            _ => bail!("Unknown select: {}", s),
        }
    }
}

#[derive(Parser)]
#[command(version, about, long_about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
#[command(arg_required_else_help = true)]
enum Commands {
    Versions {
        #[arg(value_parser=PartialArtifact::parse, help = "groupId:artifactId")]
        coordinates: PartialArtifact,
        #[arg(long, default_value_t = false, conflicts_with = "select")]
        json: bool,
        #[arg(long, conflicts_with = "json")]
        select: Option<Select>,
        #[arg(long)]
        size: Option<usize>,
    },
    ResolveFile {
        #[arg(value_parser=Artifact::parse, help = "groupId:artifactId[:packaging[:classifier]]:version"
        )]
        coordinates: Artifact,
        #[arg()]
        path: PathBuf,
    },
    ResolveProject {
        #[arg(value_parser=Artifact::parse, help = "groupId:artifactId[:packaging[:classifier]]:version"
        )]
        coordinates: Artifact,
        #[arg()]
        path: PathBuf,
        #[arg(long, default_value_t = false)]
        include_dependencies: bool,
        #[arg(long, default_value_t = false)]
        flatten: bool,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = match std::env::var("MAVEN_REPOSITORY").ok() {
        Some(s) if &s == "central" => Ok(Repository::maven_central()),
        Some(s) if &s == "central-snapshots" => Ok(Repository::maven_central_snapshots()),
        Some(r) => Url::parse(&r)
            .context(format!("Unable to parse {}", r))
            .map(Repository::both),
        None => Ok(Repository::maven_central()),
    }?;

    match cli.command {
        Some(Commands::Versions {
            coordinates,
            json,
            select,
            size,
        }) => {
            let client = make_client()?;
            let resolver = Resolver::new(&client, &repo);
            let meta = resolver.metadata(coordinates).await?;
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &meta)?;
            } else {
                match select {
                    Some(Select::Latest) => {
                        let Some(ver) = meta.versioning.latest else {
                            bail!("no latest version found");
                        };
                        println!("{ver}");
                    }
                    Some(Select::Release) => {
                        let Some(ver) = meta.versioning.release else {
                            bail!("no latest version found");
                        };
                        println!("{ver}");
                    }
                    Some(Select::Versions) => {
                        let size = size.unwrap_or(10);
                        let Some(ver) = meta.versioning.versions else {
                            bail!("no versions found");
                        };
                        let mut reversed = ver.clone();
                        reversed.reverse();
                        print!(
                            "{}",
                            reversed
                                .iter()
                                .take(size)
                                .fold(String::new(), |acc, version| {
                                    acc + version.as_ref() + "\n"
                                })
                        )
                    }
                    None => {
                        println!("{:?}", meta);
                    }
                }
            }
            Ok(())
        }
        Some(Commands::ResolveFile { coordinates, path }) => {
            if coordinates.is_project() {
                bail!("Artifact is a pom artifact, nothing to download")
            }
            if !path.is_dir() {
                bail!("Path was not an existing directory")
            }

            let client = make_client()?;
            let resolver = Resolver::new(&client, &repo);
            let file = resolver.download(&coordinates, path.as_path()).await?;
            println!("{}", file.as_path().display());
            Ok(())
        }
        Some(Commands::ResolveProject {
            coordinates,
            path,
            include_dependencies,
            flatten,
        }) => {
            let client = make_client()?;
            let resolver = Resolver::new(&client, &repo);
            if !path.is_dir() {
                bail!("Path was not an existing directory")
            }
            if coordinates.is_project() {
                bail!("Artifact is a pom artifact, nothing to download")
            }
            let lib_dir = if flatten {
                path.clone()
            } else {
                path.join("lib")
            };

            if include_dependencies {
                let deps = collect_dependencies(&resolver, &coordinates).await?;
                let joined: Result<Vec<PathBuf>, ResolveError> =
                    join_all(deps.iter().map(|a| resolver.download(a, lib_dir.as_path())))
                        .await
                        .into_iter()
                        .collect();
                for p in joined? {
                    eprintln!("{}", p.as_path().display())
                }
            }
            let file = resolver.download(&coordinates, path.as_path()).await?;
            println!("{}", file.as_path().display());
            Ok(())
        }
        None => Ok(()),
    }
}

async fn get_parent<'a>(resolver: &'a Resolver<'a>, project: &'a Project) -> Option<Project> {
    if let Some(p) = &project.parent {
        let next = resolver
            .project_metadata(ProjectReference::from(p))
            .await
            .ok()?;
        Some(next)
    } else {
        None
    }
}

async fn get_parents<'a>(resolver: &'a Resolver<'a>, project: &Project) -> Vec<Project> {
    let mut parents = vec![];
    let mut maybe_parent = get_parent(resolver, project).await;
    while let Some(p) = maybe_parent {
        parents.push(p.clone());
        maybe_parent = get_parent(resolver, &p).await
    }
    parents
}

async fn collect_dependencies<'a>(
    resolver: &'a Resolver<'a>,
    artifact: &Artifact,
) -> anyhow::Result<Vec<Artifact>> {
    let mut vec = vec![];

    let project = resolver
        .project_metadata(ProjectReference::from(artifact))
        .await?;
    let parents = get_parents(resolver, &project).await;
    let boms = get_boms_from_all(resolver, &project, &parents).await?;
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

async fn get_boms_from_all<'a>(
    resolver: &'a Resolver<'a>,
    project: &Project,
    parents: &[Project],
) -> Result<HashMap<String, Dependency>, ResolveError> {
    let mut dependencies: HashMap<String, Dependency> = HashMap::new();
    let resolved: Result<Vec<Vec<Dependency>>, ResolveError> = join_all(
        parents
            .into_iter()
            .map(|x| get_bill_of_materials(resolver, x)),
    )
    .await
    .into_iter()
    .collect();

    for parent_boms in resolved? {
        dependencies.extend(Dependency::mapped(&parent_boms))
    }

    let boms = get_bill_of_materials(resolver, project).await?;
    dependencies.extend(Dependency::mapped(&boms));

    Ok(dependencies)
}

async fn get_bill_of_materials<'a>(
    resolver: &'a Resolver<'a>,
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
            .map(|d| resolver.project_metadata(ProjectReference::from(&d.artifact))),
    )
    .await
    .into_iter()
    .collect();

    for p in all? {
        dependencies.extend(p.resolve_properties_this().dependencies);
    }

    Ok(dependencies)
}

fn make_client() -> anyhow::Result<Client> {
    let client = ClientBuilder::new().user_agent(APP_USER_AGENT);
    let auth = Authorization::from_env();
    let c = match auth {
        None => client,
        Some(Authorization::Basic { username, password }) => client.default_headers({
            let mut m = HeaderMap::new();
            let basic = BASE64_STANDARD.encode(format!("{}:{}", username, password));
            let value = HeaderValue::from_str(&format!("Basic {}", basic))?;
            m.insert(AUTHORIZATION, value);
            m
        }),
        Some(Authorization::Token { value }) => client.default_headers({
            let mut m = HeaderMap::new();
            let value = HeaderValue::from_str(&format!("Bearer {}", value))?;
            m.insert(AUTHORIZATION, value);
            m
        }),
    };

    let result = c.build()?;
    Ok(result)
}

enum Authorization {
    Basic { username: String, password: String },
    Token { value: String },
}

impl Authorization {
    fn from_env() -> Option<Authorization> {
        Self::basic().or(Self::token())
    }

    fn basic() -> Option<Authorization> {
        let username = std::env::var("MAVEN_USERNAME").ok()?;
        let password = std::env::var("MAVEN_PASSWORD").ok()?;
        Some(Authorization::Basic { username, password })
    }

    fn token() -> Option<Authorization> {
        let token = std::env::var("MAVEN_TOKEN").ok()?;
        Some(Authorization::Token { value: token })
    }
}
