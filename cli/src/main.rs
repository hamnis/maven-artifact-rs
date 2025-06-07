use anyhow;
use anyhow::Context;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use clap::{Parser, Subcommand};
use maven_artifact::Repository;
use maven_artifact::artifact::{Artifact, PartialArtifact};
use maven_artifact::resolver::Resolver;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::{Client, ClientBuilder};
use std::path::PathBuf;
use tokio;
use url::Url;

// Name your user agent after your app?
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

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
        #[arg(value_parser=PartialArtifact::parse)]
        coordinates: PartialArtifact,
    },
    Resolve {
        #[arg(value_parser=Artifact::parse)]
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
        Some(Commands::Versions { coordinates }) => {
            let result = make_client()?;
            let resolver = Resolver::new(&result, &repo);
            let meta = resolver.metadata(coordinates).await;
            println!("{:?}", meta);
            Ok(())
        }
        Some(Commands::Resolve { coordinates, path }) => {
            let result = make_client()?;
            let resolver = Resolver::new(&result, &repo);
            let file = resolver.download(coordinates, path.as_path()).await?;
            println!("{}", file.as_path().display());
            Ok(())
        }
        None => Ok(()),
    }
}

fn make_client() -> anyhow::Result<Client> {
    let client = ClientBuilder::new().user_agent(APP_USER_AGENT);
    let auth = get_auth();
    let c = match auth {
        None => client,
        Some((username, password)) => client.default_headers({
            let mut m = HeaderMap::new();
            let basic = BASE64_STANDARD.encode(format!("{}:{}", username, password));
            let value = HeaderValue::from_str(&format!("Basic {}", basic))?;
            m.insert(AUTHORIZATION, value);
            m
        }),
    };

    let result = c.build()?;
    Ok(result)
}

fn get_auth() -> Option<(String, String)> {
    let username = std::env::var("MAVEN_USERNAME").ok()?;
    let password = std::env::var("MAVEN_PASSWORD").ok()?;
    Some((username, password))
}
