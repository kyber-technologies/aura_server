use crate::connect_info::ConnectInfoInterceptor;
use crate::services::{ChatService, GeneralService, PostingService, ResourceService, UserService};
use crate::state::ServerState;
use crate::utils::{COMPRESSION, MAX_MESSAGE_SIZE};
use aura_rust::chat::v1::chat_service_server::ChatServiceServer;
use aura_rust::general::v1::general_service_server::GeneralServiceServer;
use aura_rust::posting::v1::posting_service_server::PostingServiceServer;
use aura_rust::resource::v1::resource_service_server::ResourceServiceServer;
use aura_rust::user::v1::user_service_server::UserServiceServer;
use logic::user;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use tonic::service::InterceptorLayer;
use tonic::transport::Server;
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

    tracing::info!("########## CONFIGURATION ##########");
    println!("{config:#?}");

    tracing::info!("Initializing runtime...");
    tokio::runtime::Builder::new_multi_thread()
        .enable_alt_timer()
        .enable_io()
        .max_io_events_per_tick(config.runtime.max_io_events_per_tick)
        .thread_keep_alive(Duration::from_secs(config.runtime.thread_keep_alive))
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

    tracing::info!(
        "Serving Elysium at '{}'...",
        config.network.address.as_str()
    );
    let builder = Server::builder()
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
                .accept_compressed(COMPRESSION)
                .send_compressed(COMPRESSION)
                .max_decoding_message_size(MAX_MESSAGE_SIZE)
                .max_encoding_message_size(MAX_MESSAGE_SIZE),
        )
        .add_service(
            UserServiceServer::new(UserService::new(state.clone()))
                .accept_compressed(COMPRESSION)
                .send_compressed(COMPRESSION)
                .max_decoding_message_size(MAX_MESSAGE_SIZE)
                .max_encoding_message_size(MAX_MESSAGE_SIZE),
        )
        .add_service(
            ChatServiceServer::new(ChatService::new(state.clone()))
                .accept_compressed(COMPRESSION)
                .send_compressed(COMPRESSION)
                .max_decoding_message_size(MAX_MESSAGE_SIZE)
                .max_encoding_message_size(MAX_MESSAGE_SIZE),
        )
        .add_service(
            ResourceServiceServer::new(ResourceService::new(state.clone()))
                .accept_compressed(COMPRESSION)
                .send_compressed(COMPRESSION)
                .max_decoding_message_size(MAX_MESSAGE_SIZE)
                .max_encoding_message_size(MAX_MESSAGE_SIZE),
        )
        .add_service(
            PostingServiceServer::new(PostingService::new(state.clone()))
                .accept_compressed(COMPRESSION)
                .send_compressed(COMPRESSION)
                .max_decoding_message_size(MAX_MESSAGE_SIZE)
                .max_encoding_message_size(MAX_MESSAGE_SIZE),
        );

    builder
        .serve_with_shutdown(addr, state.wait_for_exit())
        .await
        .expect("Failed to serve aura service");
}
