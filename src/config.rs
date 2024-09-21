use clap::Parser;
use serde::Deserialize;
use std::path::Path;
use std::process::exit;
use tokio::fs;
use toml;
use std::net::IpAddr;
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::*;


static DEFAULT_CONFIG_FILENAME: &str = "middleman.toml";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Starts a reverse proxy to <UPSTREAM>, listens on <BIND>:<PORT>.\nRecords upstream responses to <TAPES> directory.\nReturns recorded response if url matches (does not call upstream in this case).\n\nThe optional header `x-middleman-passthrough` can be specified in http requests to middleman to pass a request through to the <UPSTREAM>.\nAny value other than the exact string \"false\" will be considered Truthy.\nThe `--replay-only` config flag takes precedence over the `x-middleman-passthrough` header."
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
    #[arg(
        long,
        help = "The Upstream port to connect to [default: 443 when --upstream-tls]",
        default_value_t = 80
    )]
    upstream_port: u16,
    #[arg(
        long,
        help = "Should we use TLS when connection to the upstream [default: false]",
        default_value_t = false
    )]
    upstream_tls: bool,

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
    #[arg(
        long,
        help = "Should we listen for TLS connections? [default: false]",
        default_value_t = false
    )]
    listen_tls: bool,
    #[arg(
        long,
        help = "The TLS Listen Port",
        default_value_t = 5443
    )]
    tls_port: u16,
    #[arg(
        long,
        help = "The TLS cert file"
    )]
    cert_file: Option<String>,
    #[arg(
        long,
        help = "The TLS private key file"
    )]
    private_key_file: Option<String>,
}

#[derive(Deserialize, Default)]
struct TomlConfig {
    port: Option<u16>,
    upstream: Option<String>,
    tapes: Option<String>,
    bind: Option<String>,
    replay_only: Option<bool>,
    pub listen_tls: Option<bool>,
    pub tls_port: Option<u16>,
    pub cert_file: Option<String>,
    pub private_key_file: Option<String>,
    pub upstream_tls: Option<bool>,
    pub upstream_port: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub upstream: String,
    pub upstream_ip: IpAddr,
    pub upstream_tls: bool,
    pub upstream_port: u16,
    pub tapes: String,
    pub bind: String,
    pub replay_only: bool,
    pub listen_tls: bool,
    pub tls_port: u16,
    pub cert_file: Option<String>,
    pub private_key_file: Option<String>,
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

    let listen_tls = toml.listen_tls.or(Some(args.listen_tls)).or(Some(false)).unwrap();
    let cert_file = args.cert_file.or(toml.cert_file).or(None);
    let private_key_file = args.private_key_file.or(toml.private_key_file).or(None);

    if listen_tls && cert_file.is_none() {
      panic!("Trying to listen on TLS but --cert-file not provided.");
    }

    if listen_tls && private_key_file.is_none() {
      panic!("Trying to listen on TLS but --private-key-file file not provided.");
    }

    let host = args.upstream.or(toml.upstream).unwrap();
    let mut opts = ResolverOpts::default();
    // We don't want to honor the hosts file, as we want to proxy to an actual host
    opts.use_hosts_file = false;
    let resolver = TokioAsyncResolver::tokio(
        ResolverConfig::google(),
        opts);
    let upstream_ip = resolver.lookup_ip(host.clone()).await.unwrap().iter().next().expect("Cloud not resolve upstream to an ip");

    let upstream_tls = toml.upstream_tls.or(Some(args.upstream_tls)).unwrap();
    let mut upstream_port = toml.upstream_port.or(Some(args.upstream_port)).unwrap();
    if upstream_tls && upstream_port == 80 {
        // Yes... if a user actually wants to use 80 with tls, it won't work
        upstream_port = 443;
    }

    println!("Resolved {} to {}:{}", host, &upstream_ip, upstream_port);


    Config {
        listen_tls: listen_tls,
        tls_port: toml.tls_port.or(Some(args.tls_port)).unwrap(),
        cert_file: cert_file,
        private_key_file: private_key_file,

        port: args.port.or(toml.port).or(Some(5050)).unwrap(),
        upstream_ip: upstream_ip,
        upstream: host,
        upstream_tls: upstream_tls,
        upstream_port: upstream_port,
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
