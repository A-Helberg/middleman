use clap::Parser;
use serde::Deserialize;
use std::path::Path;
use std::process::exit;
use tokio::fs;
use toml;

static DEFAULT_CONFIG_FILENAME: &str = "middleman.toml";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Starts a reverse proxy to <UPSTREAM>, listens on <BIND>:<PORT>.\nRecords upstream responses to <TAPES> directory.\nReturns recorded response if url matches (does not call upstream in this case)."
)]
pub struct CliArgs {
    /// the port to listen on
    #[arg(short, long, help = "Listen port [default: 5050]")]
    port: Option<u16>,
    // Server to connect to
    #[arg(
        short,
        long,
        required = false,
        help = "The upstream host to send requests to [example: http://localhost:3000]"
    )]
    upstream: Option<String>,
    // Directory to store recordings
    #[arg(
        short,
        long,
        help = "The directory where tapes will be stored [default: ./tapes]"
    )]
    tapes: Option<String>,
    // address to bind on
    #[arg(short, long, help = "The address to bind to [default: 127.0.0.1]")]
    bind: Option<String>,
    // An override config file path
    #[arg(short, long, help="The path to a toml config file with the same options as cli", default_value_t=String::from(DEFAULT_CONFIG_FILENAME))]
    config_path: String,
    #[arg(
        long,
        help = "Only replay responses. If specified middleman will not attempt to contact the upstream",
        default_value_t = false
    )]
    replay_only: bool,
}

#[derive(Deserialize, Default)]
struct TomlConfig {
    port: Option<u16>,
    upstream: Option<String>,
    tapes: Option<String>,
    bind: Option<String>,
    replay_only: Option<bool>,
}

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub port: u16,
    pub upstream: String,
    pub tapes: String,
    pub bind: String,
    pub replay_only: bool,
}

async fn read_config(args: &CliArgs) -> TomlConfig {
    if !Path::new(&args.config_path).exists() {
        if &args.config_path != DEFAULT_CONFIG_FILENAME {
            println!("Config file ({}) specified but not found", args.config_path);
            exit(1);
        }
        return TomlConfig {
            ..Default::default()
        };
    }

    match fs::read_to_string(&args.config_path).await {
        Ok(contents) => {
            let x: TomlConfig = match toml::from_str(&contents) {
                Ok(toml_args) => toml_args,
                Err(_) => {
                    eprintln!("Unable to load configuration from `{}`", args.config_path);
                    exit(1);
                }
            };
            return x;
        }
        Err(_) => {
            eprintln!(
                "Unable to read the configuration file `{}`",
                args.config_path
            );
            exit(1);
        }
    }
}
pub async fn get_config() -> Config {
    let args = CliArgs::parse();
    let toml = read_config(&args).await;

    validate(&args, &toml);

    Config {
        port: args.port.or(toml.port).or(Some(5050)).unwrap(),
        upstream: args.upstream.or(toml.upstream).unwrap(),
        tapes: args
            .tapes
            .or(toml.tapes)
            .or(Some("tapes".to_string()))
            .unwrap(),
        bind: args
            .bind
            .or(toml.bind)
            .or(Some("127.0.0.1".to_string()))
            .unwrap(),
        replay_only: toml.replay_only.or(Some(args.replay_only)).unwrap(),
    }
}

fn validate(args: &CliArgs, toml: &TomlConfig) {
    if args.upstream.clone().or(toml.upstream.clone()).is_none() {
        eprintln!("You did not provide an upstream");
        exit(1);
    }
}
