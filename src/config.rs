use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn init() {
    let config = if cfg!(test) {
        Config::default()
    } else {
        let path = std::env::var("CONFIG_FILE").unwrap_or_else(|_| {
            let path = if cfg!(debug_assertions) {
                "./dev/config.toml"
            } else {
                "./config.toml"
            }
            .to_string();

            if !std::fs::exists(&path).expect("Failed to check if config file exists") {
                println!("Writing default config...");
                std::fs::write(
                    &path,
                    serde_json::to_string_pretty(&Config::default())
                        .expect("Failed to serialize default config"),
                )
                .expect("Failed to write default config file");
            }

            path
        });

        let raw = std::fs::read(path).expect("Failed to read config file");

        serde_json::from_slice(&raw).expect("Failed to deserialize config")
    };

    if !std::fs::exists(&config.service.resource_dir)
        .expect("Failed to check if resource directory exists")
    {
        std::fs::create_dir(&config.service.resource_dir)
            .expect("Failed to create resource directory");
    }

    CONFIG.set(config).expect("Failed to set config");
}

pub fn get<'a>() -> &'a Config {
    CONFIG.get().expect("Failed to get config")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub runtime: RuntimeConfig,
    pub service: ServiceConfig,
    pub network: NetworkConfig,
    pub database: DatabaseConfig,
    pub email: EmailConfig,
    pub log: LogConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub max_io_events_per_tick: usize,
    pub thread_keep_alive: u64,
    pub global_queue_interval: u32,
    pub event_interval: u32,
    pub worker_threads: usize,
    pub max_blocking_threads: usize,
    pub maintain_interval: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_io_events_per_tick: 1024,
            thread_keep_alive: 10,
            global_queue_interval: 31,
            event_interval: 61,
            worker_threads: 4,
            max_blocking_threads: 256,
            maintain_interval: 60 * 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub public_key: String,
    pub private_key: String,
    pub token_expiration: u64,
    pub item_request_limit: u32,
    pub notification_expiration_time: i64,
    pub resource_dir: String,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            public_key: if cfg!(debug_assertions) {
                "./dev/public-key.pem"
            } else {
                "./secure/public-key.pem"
            }
            .to_string(),
            private_key: if cfg!(debug_assertions) {
                "./dev/private-key.pem"
            } else {
                "./secure/private-key.pem"
            }
            .to_string(),
            item_request_limit: 50,
            token_expiration: 168,
            notification_expiration_time: 144,
            resource_dir: if cfg!(debug_assertions) {
                "./dev/resources"
            } else {
                "./resources"
            }
            .to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub address: String,
    pub rate_limit_replenish: u64,
    pub rate_limit_burst: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:50051".to_string(),
            rate_limit_replenish: 100,
            rate_limit_burst: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub max_pool_size: usize,
    pub log: bool,
    pub embedding_max_length: usize,
    pub embedding_threads: usize,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            user: "postgres".to_string(),
            password: if cfg!(debug_assertions) {
                "./dev/database-password"
            } else {
                "./secure/database-password"
            }
            .to_string(),
            host: "127.0.0.1".to_string(),
            port: 5432,
            max_pool_size: 16,
            log: cfg!(debug_assertions),
            embedding_max_length: 256,
            embedding_threads: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp: String,
    pub smtp_user: String,
    pub smtp_password: String,
    pub exp: u64,
    pub no_reply_mail: String,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            smtp: "127.0.0.1:1025".to_string(), // Default Mailhog SMTP Server
            smtp_user: "user".to_string(),
            smtp_password: if cfg!(debug_assertions) {
                "./dev/smtp-password"
            } else {
                "./secure/smtp-password"
            }
            .to_string(),
            exp: 3600,
            no_reply_mail: "no-reply@aura.social".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub file_names: bool,
    pub targets: bool,
    pub level: String,
    pub threads: bool,
    pub time: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            file_names: false,
            targets: false,
            level: "info".to_string(),
            threads: false,
            time: false,
        }
    }
}
