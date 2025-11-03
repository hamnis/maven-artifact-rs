use anyhow::{Context, bail};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use clap::{Parser, Subcommand};
use maven_artifact::Repository;
use maven_artifact::artifact::{Artifact, PartialArtifact};
use maven_artifact::resolver::Resolver;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::{Client, ClientBuilder};
use std::path::PathBuf;
use std::str::FromStr;
use tokio;
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
    Resolve {
        #[arg(value_parser=Artifact::parse, help = "groupId:artifactId[:packaging[:classifier]]:version"
        )]
        coordinates: Artifact,
        #[arg()]
        path: PathBuf,
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
                                    acc + &version.to_string() + "\n"
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
        Some(Commands::Resolve { coordinates, path }) => {
            let client = make_client()?;
            let resolver = Resolver::new(&client, &repo);
            let file = resolver.download(coordinates, path.as_path()).await?;
            println!("{}", file.as_path().display());
            Ok(())
        }
        None => Ok(()),
    }
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
