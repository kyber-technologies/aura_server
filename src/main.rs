use crate::connect_info::ConnectInfoInterceptor;
use crate::services::{ChatService, GeneralService, PostingService, ResourceService, UserService};
use crate::state::ServerState;
use aura_rust::chat::v1::chat_service_server::ChatServiceServer;
use aura_rust::general::v1::general_service_server::GeneralServiceServer;
use aura_rust::posting::v1::posting_service_server::PostingServiceServer;
use aura_rust::resource::v1::resource_service_server::ResourceServiceServer;
use aura_rust::user::v1::user_service_server::UserServiceServer;
use logic::user;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use tonic::codec::CompressionEncoding;
use tonic::service::InterceptorLayer;
use tonic::transport::{Identity, Server, ServerTlsConfig};
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tracing::level_filters::LevelFilter;

mod auth;
mod config;
mod connect_info;
mod console;
mod database;
mod email;
mod embedder;
mod error;
mod logic;
mod schema;
mod services;
mod state;
mod types;
mod utils;

#[cfg(feature = "testing")]
mod testing;

fn main() {
    println!("Loading configuration...");
    config::init();

    let config = config::get();

    let (mut readline, stdout) = console::create();

    let logger = tracing_subscriber::FmtSubscriber::builder()
        .with_file(config.log.file_names)
        .with_target(config.log.targets)
        .with_thread_names(config.log.threads)
        .with_thread_ids(config.log.threads)
        .with_max_level(
            LevelFilter::from_str(config.log.level.as_str()).expect("Invalid log level"),
        )
        .with_writer(stdout);

    if !config.log.time {
        logger.without_time().init();
    } else {
        logger.init();
    }

    tracing::info!("Logger initialized!");

    tracing::info!("Installing Crypto Provider...");
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install default crypto provider");

    tracing::info!("########## CONFIGURATION ##########");
    println!("{config:#?}");

    tracing::info!("Initializing runtime...");
    tokio::runtime::Builder::new_multi_thread()
        .enable_alt_timer()
        .enable_io()
        .max_io_events_per_tick(config.runtime.max_io_events_per_tick)
        .thread_keep_alive(config.runtime.thread_keep_alive)
        .global_queue_interval(config.runtime.global_queue_interval)
        .event_interval(config.runtime.event_interval)
        .worker_threads(config.runtime.worker_threads)
        .max_blocking_threads(config.runtime.max_blocking_threads)
        .thread_name("aura-worker")
        .build()
        .expect("Failed to build tokio runtime")
        .block_on(async {
            tracing::info!("Initializing authentication...");
            auth::init().await;

            tracing::info!("Initializing server state...");
            let state = ServerState::create().await;

            tokio::select! {
                _ = serve(state.clone()) => (),
                _ = console::run(&mut readline, state.clone()) => (),
                _ = state.maintain() => (),
            }

            println!("\nShutting down Aura...");

            state.dispose();
        });
}

async fn serve(state: ServerState) {
    let config = config::get();

    let addr =
        SocketAddr::from_str(config.network.address.as_str()).expect("Failed to parse address");

    // Create initial admin if not present
    user::create_admin(
        &mut state
            .database()
            .await
            .expect("Failed to get database connection"),
    )
    .await
    .expect("Failed to create admin user");

    let tls_config = {
        let tls_cert = tokio::fs::read_to_string(&config.network.tls_certificate)
            .await
            .expect("Failed to read TLS certificate");

        let tls_key = tokio::fs::read_to_string(&config.network.tls_key)
            .await
            .expect("Failed to read TLS key");

        let identity = Identity::from_pem(tls_cert, tls_key);

        ServerTlsConfig::new()
            .timeout(Duration::from_secs(config.network.tls_timeout))
            .identity(identity)
            .ignore_client_order(true)
    };

    tracing::info!(
        "Serving Aura gRPC Service at '{}'...",
        config.network.address.as_str()
    );
    let builder = Server::builder()
        .timeout(Duration::from_secs(config.network.timeout))
        .concurrency_limit_per_connection(config.network.concurrency_limit_per_connection)
        .max_concurrent_streams(config.network.max_concurrent_streams)
        .max_connection_age(config.network.max_connection_age)
        .max_connection_age_grace(config.network.max_connection_age_grace)
        .http2_keepalive_interval(Some(config.network.http2_keepalive_interval))
        .http2_keepalive_timeout(Some(config.network.http2_keepalive_timeout))
        .tcp_keepalive(Some(config.network.tcp_keepalive))
        .tcp_keepalive_interval(Some(config.network.tcp_keepalive_interval))
        .tcp_keepalive_retries(Some(config.network.tcp_keepalive_retries))
        .load_shed(true)
        .tcp_nodelay(true)
        .http2_adaptive_window(Some(true))
        .initial_stream_window_size(1024 * 1024) // 1 MB
        .http2_max_header_list_size(1024 * 16) // 16 KB
        .max_frame_size(1024 * 16) // 16 KB
        .http2_max_pending_accept_reset_streams(Some(20))
        .http2_max_local_error_reset_streams(Some(20))
        .accept_http1(false)
        .tls_config(tls_config)
        .expect("Failed to build TLS config")
        .layer(InterceptorLayer::new(ConnectInfoInterceptor))
        .layer(GovernorLayer::new(
            GovernorConfigBuilder::default()
                .key_extractor(SmartIpKeyExtractor)
                .per_millisecond(config.network.rate_limit_replenish)
                .burst_size(config.network.rate_limit_burst)
                .finish()
                .expect("Failed to build governor config"),
        ))
        .add_service(
            GeneralServiceServer::new(GeneralService::new(state.clone()))
                .accept_compressed(CompressionEncoding::Zstd)
                .max_decoding_message_size(config.service.max_message_size)
                .max_encoding_message_size(config.service.max_message_size),
        )
        .add_service(
            UserServiceServer::new(UserService::new(state.clone()))
                .accept_compressed(CompressionEncoding::Zstd)
                .max_decoding_message_size(config.service.max_message_size)
                .max_encoding_message_size(config.service.max_message_size),
        )
        .add_service(
            ChatServiceServer::new(ChatService::new(state.clone()))
                .accept_compressed(CompressionEncoding::Zstd)
                .max_decoding_message_size(config.service.max_message_size)
                .max_encoding_message_size(config.service.max_message_size),
        )
        .add_service(
            ResourceServiceServer::new(ResourceService::new(state.clone()))
                .accept_compressed(CompressionEncoding::Zstd)
                // Send compressed responses for resource service
                .send_compressed(CompressionEncoding::Zstd)
                .max_decoding_message_size(config.service.max_message_size)
                .max_encoding_message_size(config.service.max_message_size),
        )
        .add_service(
            PostingServiceServer::new(PostingService::new(state.clone()))
                .accept_compressed(CompressionEncoding::Zstd)
                .max_decoding_message_size(config.service.max_message_size)
                .max_encoding_message_size(config.service.max_message_size),
        );

    builder
        .serve_with_shutdown(addr, state.wait_for_exit())
        .await
        .expect("Failed to serve aura service");
}
