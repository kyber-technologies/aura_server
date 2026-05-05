use crate::config;
use std::str::FromStr;
use tracing::level_filters::LevelFilter;

pub fn init_logger() {
    let config = config::get();

    let builder = tracing_subscriber::FmtSubscriber::builder()
        .with_file(config.log_file_names)
        .with_target(config.log_targets)
        .with_thread_names(config.log_threads)
        .with_thread_ids(config.log_threads)
        .with_max_level(
            LevelFilter::from_str(config.log_level.as_str()).expect("Invalid log level"),
        );

    if !config.log_time {
        builder.without_time().init();
    } else {
        builder.init();
    }
}
