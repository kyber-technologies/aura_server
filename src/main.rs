use crate::connect_info::ConnectInfoInterceptor;
use crate::services::{ChatService, GeneralService, ResourceService, UserService};
use crate::state::ServerState;
use crate::utils::{COMPRESSION, MAX_MESSAGE_SIZE};
use aura_rust::chat::v1::chat_service_server::ChatServiceServer;
use aura_rust::general::v1::general_service_server::GeneralServiceServer;
use aura_rust::resource::v1::resource_service_server::ResourceServiceServer;
use aura_rust::user::v1::user_service_server::UserServiceServer;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use tonic::service::InterceptorLayer;
use tonic::transport::Server;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;

mod auth;
mod chat;
mod config;
mod connect_info;
mod database;
mod email;
mod error;
mod resource;
mod services;
mod state;
mod trace;
mod user;
mod utils;

fn main() {
    println!("Loading configuration...");
    config::init();
    let config = config::get();

    trace::init_logger();
    tracing::info!("Logger initialized!");

    tracing::info!("########## CONFIGURATION ##########");
    println!("{config:#?}");

    tracing::info!("Initializing runtime...");
    tokio::runtime::Builder::new_multi_thread()
        .enable_alt_timer()
        .enable_io()
        .max_io_events_per_tick(config.rt_max_io_events_per_tick)
        .thread_keep_alive(Duration::from_secs(config.rt_thread_keep_alive))
        .global_queue_interval(config.rt_global_queue_interval)
        .event_interval(config.rt_event_interval)
        .worker_threads(config.rt_worker_threads)
        .max_blocking_threads(config.rt_max_blocking_threads)
        .thread_name("aura-worker")
        .build()
        .expect("Failed to build tokio runtime")
        .block_on(async {
            tracing::info!("Initializing authentication...");
            auth::init().await;

            tracing::info!("Initializing server state...");
            let state = ServerState::new().await;

            tokio::select! {
                _ = serve(state.clone()) => (),
                _ = exit_signal(state) => (),
            }
        });
}

async fn serve(state: ServerState) {
    let config = config::get();

    let addr = SocketAddr::from_str(config.net_address.as_str()).expect("Failed to parse address");

    tracing::info!("Launching maintenance loop...");
    let state2 = state.clone();
    tokio::task::spawn(async move {
        state2.maintain().await;
    });

    // Create initial admin if not present
    user::create_admin(state.database())
        .await
        .expect("Failed to create admin user");

    tracing::info!("Creating reflection server...");
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(aura_rust::FILE_DESCRIPTOR_SET)
        .include_reflection_service(true)
        .build_v1alpha()
        .expect("Failed to build reflection server");

    tracing::info!("Serving Elysium at '{}'...", config.net_address.as_str());
    let builder = Server::builder()
        .layer(InterceptorLayer::new(ConnectInfoInterceptor))
        .layer(GovernorLayer::new(
            GovernorConfigBuilder::default()
                .key_extractor(SmartIpKeyExtractor)
                .per_millisecond(config.net_rate_limit_replenish)
                .burst_size(config.net_rate_limit_burst)
                .finish()
                .expect("Failed to build governor config"),
        ))
        .add_service(reflection)
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
        );

    builder
        .serve(addr)
        .await
        .expect("Failed to serve aura service");
}

async fn exit_signal(state: ServerState) {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to get Ctrl+C signal");

    state.set_exit();

    tracing::info!("Shutting down aura...");
}
