//
//
// main.rs
// Copyright (C) 2022 db3.network Author imotai <codego.me@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
//
//
use shadow_rs::shadow;
shadow!(build);
use actix_cors::Cors;
use actix_web::{rt, web, App, HttpServer};
use clap::{Parser, Subcommand};
use db3_crypto::signer::Db3Signer;
use db3_node::abci_impl::{AbciImpl, NodeState};
use db3_node::auth_storage::AuthStorage;
use db3_node::json_rpc_impl;
use db3_node::storage_node_impl::StorageNodeImpl;
use db3_proto::db3_node_proto::storage_node_client::StorageNodeClient;
use db3_proto::db3_node_proto::storage_node_server::StorageNodeServer;
use db3_sdk::mutation_sdk::MutationSDK;
use db3_sdk::store_sdk::StoreSDK;
use http::Uri;
use merk::Merk;
use std::io::stdout;
use std::io::Write;
use std::io::{self, BufRead};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use tendermint_abci::ServerBuilder;
use tendermint_rpc::HttpClient;
use tonic::transport::{ClientTlsConfig, Endpoint, Server};
use tracing::info;
use tracing_subscriber::filter::LevelFilter;

const ABOUT: &str = "
██████╗ ██████╗ ██████╗ 
██╔══██╗██╔══██╗╚════██╗
██║  ██║██████╔╝ █████╔╝
██║  ██║██╔══██╗ ╚═══██╗
██████╔╝██████╔╝██████╔╝
╚═════╝ ╚═════╝ ╚═════╝ 
@db3.network🚀🚀🚀";

#[derive(Debug, Parser)]
#[clap(name = "db3")]
#[clap(about = ABOUT, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start a interactive shell
    #[clap()]
    Shell {
        /// the url of db3 grpc api
        #[clap(long, default_value = "http://127.0.0.1:26659")]
        public_grpc_url: String,
        /// the broadcast url of db3 json rpc api
        #[clap(long, default_value = "http://127.0.0.1:26657")]
        public_json_rpc_url: String,
    },

    /// Start DB3 node server
    #[clap()]
    Node {
        /// Bind the gprc server to this .
        #[clap(long, default_value = "127.0.0.1")]
        public_host: String,
        /// The port of grpc api
        #[clap(long, default_value = "26659")]
        public_grpc_port: u16,
        #[clap(long, default_value = "26670")]
        public_json_rpc_port: u16,
        /// Bind the abci server to this port.
        #[clap(long, default_value = "26658")]
        abci_port: u16,
        /// The porf of tendemint
        #[clap(long, default_value = "26657")]
        tm_port: u16,
        /// The default server read buffer size, in bytes, for each incoming client
        /// connection.
        #[clap(short, long, default_value = "1048576")]
        read_buf_size: usize,
        /// Increase output logging verbosity to DEBUG level.
        #[clap(short, long)]
        verbose: bool,
        /// Suppress all output logging (overrides --verbose).
        #[clap(short, long)]
        quiet: bool,
        #[clap(short, long, default_value = "./db")]
        db_path: String,
        /// disable grpc-web
        #[clap(long, default_value = "false")]
        disable_grpc_web: bool,
    },

    /// Get the version of DB3
    #[clap()]
    Version {},
}

///  start abci server
fn start_abci_service(
    abci_port: u16,
    read_buf_size: usize,
    store: Arc<Mutex<Pin<Box<AuthStorage>>>>,
) -> Arc<NodeState> {
    let addr = format!("{}:{}", "127.0.0.1", abci_port);
    let abci_impl = AbciImpl::new(store);
    let node_state = abci_impl.get_node_state().clone();
    thread::spawn(move || {
        let server = ServerBuilder::new(read_buf_size)
            .bind(addr, abci_impl)
            .unwrap();
        server.listen().unwrap();
    });
    node_state
}

fn start_json_rpc_service(
    public_host: &str,
    public_json_rpc_port: u16,
    context: json_rpc_impl::Context,
) {
    let local_public_host = public_host.to_string();
    let addr = format!("{}:{}", local_public_host, public_json_rpc_port);
    info!("start json rpc server with addr {}", addr.as_str());
    thread::spawn(move || {
        rt::System::new()
            .block_on(async {
                HttpServer::new(move || {
                    let cors = Cors::default()
                        .allow_any_origin()
                        .allow_any_method()
                        .allow_any_header()
                        .max_age(3600);
                    App::new()
                        .app_data(web::Data::new(context.clone()))
                        .wrap(cors)
                        .service(
                            web::resource("/").route(web::post().to(json_rpc_impl::rpc_router)),
                        )
                })
                .bind((local_public_host, public_json_rpc_port))
                .unwrap()
                .run()
                .await
            })
            .unwrap();
    });
}

async fn start_node(cmd: Commands) {
    if let Commands::Node {
        public_host,
        public_grpc_port,
        public_json_rpc_port,
        abci_port,
        tm_port,
        read_buf_size,
        verbose,
        quiet,
        db_path,
        disable_grpc_web,
    } = cmd
    {
        let log_level = if quiet {
            LevelFilter::OFF
        } else if verbose {
            LevelFilter::DEBUG
        } else {
            LevelFilter::INFO
        };
        tracing_subscriber::fmt().with_max_level(log_level).init();
        info!("{}", ABOUT);
        let merk = Merk::open(&db_path).unwrap();
        let store = Arc::new(Mutex::new(Box::pin(AuthStorage::new(merk))));
        match store.lock() {
            Ok(mut s) => {
                s.init().unwrap();
            }
            _ => todo!(),
        }
        //TODO recover storage
        let store_for_abci = store.clone();
        let _node_state = start_abci_service(abci_port, read_buf_size, store_for_abci);
        let tm_addr = format!("http://127.0.0.1:{}", tm_port);
        info!("db3 json rpc server will connect to tendermint {}", tm_addr);
        let client = HttpClient::new(tm_addr.as_str()).unwrap();
        let context = json_rpc_impl::Context {
            store: store.clone(),
            client,
        };
        start_json_rpc_service(&public_host, public_json_rpc_port, context);
        let addr = format!("{}:{}", public_host, public_grpc_port);
        let storage_node = StorageNodeImpl::new(store);
        info!("start db3 storage node on public addr {}", addr);
        if disable_grpc_web {
            Server::builder()
                .add_service(StorageNodeServer::new(storage_node))
                .serve(addr.parse().unwrap())
                .await
                .unwrap();
        } else {
            Server::builder()
                .accept_http1(true)
                .add_service(tonic_web::enable(StorageNodeServer::new(storage_node)))
                .serve(addr.parse().unwrap())
                .await
                .unwrap();
        }
    }
}

async fn start_shell(cmd: Commands) {
    if let Commands::Shell {
        public_grpc_url,
        public_json_rpc_url,
    } = cmd
    {
        println!("{}", ABOUT);
        let kp = db3_cmd::get_key_pair(true).unwrap();
        // broadcast client
        let client = HttpClient::new(public_json_rpc_url.as_str()).unwrap();
        let signer = Db3Signer::new(kp);
        let sdk = MutationSDK::new(client, signer);
        let kp = db3_cmd::get_key_pair(false).unwrap();
        let signer = Db3Signer::new(kp);
        let uri = public_grpc_url.parse::<Uri>().unwrap();
        let endpoint = match uri.scheme_str() == Some("https") {
            true => {
                let rpc_endpoint = Endpoint::new(public_grpc_url)
                    .unwrap()
                    .tls_config(ClientTlsConfig::new())
                    .unwrap();
                rpc_endpoint
            }
            false => {
                let rpc_endpoint = Endpoint::new(public_grpc_url).unwrap();
                rpc_endpoint
            }
        };
        let channel = endpoint.connect_lazy();
        let client = Arc::new(StorageNodeClient::new(channel));
        let store_sdk = StoreSDK::new(client, signer);
        print!(">");
        stdout().flush().unwrap();
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Err(_) => {
                    return;
                }
                Ok(s) => {
                    db3_cmd::process_cmd(&sdk, &store_sdk, s.as_str()).await;
                    print!(">");
                    stdout().flush().unwrap();
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    match args.command {
        Commands::Shell { .. } => start_shell(args.command).await,
        Commands::Node { .. } => start_node(args.command).await,
        Commands::Version { .. } => {
            if shadow_rs::tag().len() > 0 {
                println!("version:{}", shadow_rs::tag());
            } else {
                println!(
                    "warning: a development version being used in branch {}",
                    shadow_rs::branch()
                );
            }
            println!("commit:{}", build::SHORT_COMMIT);
        }
    }
}