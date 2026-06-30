use std::path::Path;
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
                std::fs::write(&path, Config::default().write())
                    .expect("Failed to write default config file");
            }

            path
        });

        Config::parse(path)
    };

    if !std::fs::exists(&config.service_resource_dir)
        .expect("Failed to check if resource directory exists")
    {
        std::fs::create_dir(&config.service_resource_dir)
            .expect("Failed to create resource directory");
    }

    CONFIG.set(config).expect("Failed to set config");
}

pub fn get<'a>() -> &'a Config {
    CONFIG.get().expect("Failed to get config")
}

#[derive(Debug, Clone)]
pub struct Config {
    pub service_public_key: String,
    pub service_private_key: String,
    pub service_token_expiration: u64,
    pub service_max_search_results: usize,
    pub service_notification_expiration_time: u64,
    pub service_resource_dir: String,
    pub net_address: String,
    pub net_rate_limit_replenish: u64,
    pub net_rate_limit_burst: u32,
    pub rt_max_io_events_per_tick: usize,
    pub rt_thread_keep_alive: u64,
    pub rt_global_queue_interval: u32,
    pub rt_event_interval: u32,
    pub rt_worker_threads: usize,
    pub rt_max_blocking_threads: usize,
    pub rt_maintain_interval: u64,
    pub db_address: String,
    pub db_user: String,
    pub db_password: String,
    pub email_smtp: String,
    pub email_smtp_user: String,
    pub email_smtp_password: String,
    pub email_exp: u64,
    pub email_no_reply_mail: String,
    pub log_file_names: bool,
    pub log_targets: bool,
    pub log_level: String,
    pub log_threads: bool,
    pub log_time: bool,
}

impl Config {
    pub fn parse(file: impl AsRef<Path>) -> Self {
        let data = std::fs::read_to_string(file).expect("Failed to read config file");

        let toml = boml::parse(&data).expect("Failed to parse config file");

        let service = toml
            .get_table("service")
            .expect("Failed parsing 'service' table");

        let service_public_key = service
            .get_string("public_key")
            .expect("Failed parsing 'service.public_key' field")
            .to_string();

        let service_private_key = service
            .get_string("private_key")
            .expect("Failed parsing 'service.private_key' field")
            .to_string();

        let service_max_search_results = service
            .get_integer("max_search_results")
            .expect("Failed parsing 'service.max_search_results' field")
            as usize;

        let service_token_expiration = service
            .get_integer("token_expiration")
            .expect("Failed parsing 'service.token_expiration' field")
            as u64;

        let service_notification_expiration_time = service
            .get_integer("notification_expiration_time")
            .expect("Failed parsing 'service.notification_expiration_time' field")
            as u64;

        let service_resource_dir = service
            .get_string("resource_dir")
            .expect("Failed parsing 'service.resource_dir' field")
            .to_string();

        let network = toml
            .get_table("network")
            .expect("Failed parsing 'network' table");

        let net_address = network
            .get_string("address")
            .expect("Failed parsing 'network.address' field")
            .to_string();

        let net_rate_limit_replenish = network
            .get_integer("rate_limit_replenish")
            .expect("Failed parsing 'network.rate_limit_replenish' field")
            as u64;

        let net_rate_limit_burst = network
            .get_integer("rate_limit_burst")
            .expect("Failed parsing 'network.rate_limit_burst' field")
            as u32;

        let runtime = toml
            .get_table("runtime")
            .expect("Failed parsing 'runtime' table");

        let rt_max_io_events_per_tick = runtime
            .get_integer("max_io_events_per_tick")
            .expect("Failed parsing 'runtime.max_io_events_per_tick' field")
            as usize;

        let rt_thread_keep_alive = runtime
            .get_integer("thread_keep_alive")
            .expect("Failed parsing 'runtime.thread_keep_alive' field")
            as u64;

        let rt_global_queue_interval = runtime
            .get_integer("global_queue_interval")
            .expect("Failed parsing 'runtime.global_queue_interval' field")
            as u32;

        let rt_event_interval = runtime
            .get_integer("event_interval")
            .expect("Failed parsing 'runtime.event_interval' field")
            as u32;

        let rt_worker_threads = runtime
            .get_integer("worker_threads")
            .expect("Failed parsing 'runtime.worker_threads' field")
            as usize;

        let rt_max_blocking_threads = runtime
            .get_integer("max_blocking_threads")
            .expect("Failed parsing 'runtime.max_blocking_threads' field")
            as usize;

        let rt_maintain_interval = runtime
            .get_integer("maintain_interval")
            .expect("Failed parsing 'runtime.maintain_interval' field")
            as u64;

        let database = toml
            .get_table("database")
            .expect("Failed parsing 'database' table");

        let db_address = database
            .get_string("address")
            .expect("Failed parsing 'database.address' field")
            .to_string();

        let db_user = database
            .get_string("user")
            .expect("Failed parsing 'database.user' field")
            .to_string();

        let db_password = database
            .get_string("password")
            .expect("Failed parsing 'database.password' field")
            .to_string();

        let email = toml
            .get_table("email")
            .expect("Failed parsing 'email' table");

        let email_smtp = email
            .get_string("smtp")
            .expect("Failed parsing 'email.smtp' table")
            .to_string();

        let email_smtp_user = email
            .get_string("smtp_user")
            .expect("Failed parsing 'email.smtp_user' field")
            .to_string();

        let email_smtp_password = email
            .get_string("smtp_password")
            .expect("Failed parsing 'email.smtp_password' field")
            .to_string();

        let email_exp = email
            .get_integer("expiration")
            .expect("Failed parsing 'email.expiration' field") as u64;

        let email_no_reply_mail = email
            .get_string("no_reply_mail")
            .expect("Failed parsing 'email.no_reply_mail' field")
            .to_string();

        let logging = toml
            .get_table("logging")
            .expect("Failed parsing 'logging' table");

        let log_file_names = logging
            .get_boolean("file_names")
            .expect("Failed parsing 'logging.file_names' field");

        let log_targets = logging
            .get_boolean("targets")
            .expect("Failed parsing 'logging.targets' field");

        let log_level = logging
            .get_string("level")
            .expect("Failed parsing 'logging.level' field")
            .to_string();

        let log_threads = logging
            .get_boolean("threads")
            .expect("Failed parsing 'logging.threads' field");

        let log_time = logging
            .get_boolean("time")
            .expect("Failed parsing 'logging.time' field");

        Config {
            service_public_key,
            service_private_key,
            service_max_search_results,
            service_token_expiration,
            service_notification_expiration_time,
            service_resource_dir,
            net_address,
            net_rate_limit_replenish,
            net_rate_limit_burst,
            rt_max_io_events_per_tick,
            rt_thread_keep_alive,
            rt_global_queue_interval,
            rt_event_interval,
            rt_worker_threads,
            rt_max_blocking_threads,
            rt_maintain_interval,
            db_address,
            db_user,
            db_password,
            email_smtp,
            email_smtp_user,
            email_smtp_password,
            email_exp,
            email_no_reply_mail,
            log_file_names,
            log_targets,
            log_level,
            log_threads,
            log_time,
        }
    }

    pub fn database_password(&self) -> String {
        std::fs::read_to_string(&self.db_password)
            .expect("Failed to read database password file")
            .trim()
            .to_string()
    }

    pub fn write(&self) -> String {
        let Config {
            service_public_key,
            service_private_key,
            service_max_search_results,
            service_token_expiration,
            service_notification_expiration_time,
            service_resource_dir,
            net_address,
            net_rate_limit_replenish,
            net_rate_limit_burst,
            rt_max_io_events_per_tick,
            rt_thread_keep_alive,
            rt_global_queue_interval,
            rt_event_interval,
            rt_worker_threads,
            rt_max_blocking_threads,
            rt_maintain_interval,
            db_address,
            db_user,
            db_password,
            email_smtp,
            email_smtp_user,
            email_smtp_password,
            email_exp,
            email_no_reply_mail,
            log_file_names,
            log_targets,
            log_level,
            log_threads,
            log_time,
        } = self;

        format!(
            r#"# Elysium Configuration File

[service]
# Path to the public EdDSA key used for JWT authentication.
public_key = "{service_public_key}"
# Path to the private EdDSA key used for JWT authentication.
private_key = "{service_private_key}"
# Maximum number of search results returned search requests.
max_search_results = {service_max_search_results}
# Token expiration time in hours.
token_expiration = {service_token_expiration}
# Notification expiration time in hours.
notification_expiration_time = {service_notification_expiration_time}
# Directory where uploaded resources are stored.
resource_dir = "{service_resource_dir}"

[network]
# Address of the gRPC service.
address = "{net_address}"
# Rate limit token replenish rate in milliseconds.
rate_limit_replenish = {net_rate_limit_replenish}
# Rate limit token burst size.
rate_limit_burst = {net_rate_limit_burst}

[runtime]
# Maximum I/O events processed per tick.
max_io_events_per_tick = {rt_max_io_events_per_tick}
# Thread keep alive time in seconds.
thread_keep_alive = {rt_thread_keep_alive}
# Global queue interval.
global_queue_interval = {rt_global_queue_interval}
# Event interval.
event_interval = {rt_event_interval}
# Worker threads to use.
worker_threads = {rt_worker_threads}
# Maximum threads to spawn for blocking operations.
max_blocking_threads = {rt_max_blocking_threads}
# Maintenance interval in seconds.
maintain_interval = {rt_maintain_interval}

[database]
# Address to the SurrealDB database server.
address = "{db_address}"
# The user to log into SurrealDB with.
user = "{db_user}"
# Path to the file containing the password for the SurrealDB user.
password = "{db_password}"

[email]
# SMTP server address.
smtp = "{email_smtp}"
# SMTP server username.
smtp_user = "{email_smtp_user}"
# Path to the file containing the password for the SMTP user.
smtp_password = "{email_smtp_password}"
# Email expiration time in seconds.
expiration = {email_exp}
# Email to send from.
no_reply_mail = "{email_no_reply_mail}"

[logging]
# Should log records contain file names.
file_names = {log_file_names}
# Should log records containg target names.
targets = {log_targets}
# The most-verbose level of logging to start logging at.
level = "{log_level}"
# Should log records contain thread IDs and names.
threads = {log_threads}
# Should log records contain timestamps.
time = {log_time}
        "#
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            service_public_key: if cfg!(debug_assertions) {
                "./dev/public-key.pem"
            } else {
                "./secure/public-key.pem"
            }
            .to_string(),
            service_private_key: if cfg!(debug_assertions) {
                "./dev/private-key.pem"
            } else {
                "./secure/private-key.pem"
            }
            .to_string(),
            service_max_search_results: 50,
            service_token_expiration: 168,
            service_notification_expiration_time: 144,
            service_resource_dir: if cfg!(debug_assertions) {
                "./dev/resources"
            } else {
                "./resources"
            }
            .to_string(),
            net_address: "127.0.0.1:50051".to_string(),
            net_rate_limit_replenish: 100,
            net_rate_limit_burst: 30,
            rt_max_io_events_per_tick: 1024,
            rt_thread_keep_alive: 10,
            rt_global_queue_interval: 31,
            rt_event_interval: 61,
            rt_worker_threads: 4,
            rt_max_blocking_threads: 256,
            rt_maintain_interval: 60 * 10,
            db_address: "127.0.0.1:8000".to_string(),
            db_user: "root".to_string(),
            db_password: if cfg!(debug_assertions) {
                "./dev/database-password"
            } else {
                "./secure/database-password"
            }
            .to_string(),
            email_smtp: "127.0.0.1:1025".to_string(), // Default Mailhog SMTP Server
            email_smtp_user: "user".to_string(),
            email_smtp_password: if cfg!(debug_assertions) {
                "./dev/smtp-password"
            } else {
                "./secure/smtp-password"
            }
            .to_string(),
            email_exp: 3600,
            email_no_reply_mail: "no-reply@aura.social".to_string(),
            log_file_names: false,
            log_targets: false,
            log_level: "info".to_string(),
            log_threads: false,
            log_time: false,
        }
    }
}
