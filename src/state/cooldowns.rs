use crate::error::Error;
use crate::types::{FastDashMap, FastMap};
use crate::{config, utils};
use dashmap::Entry;
use serde::de::IntoDeserializer;
use serde::de::value::StringDeserializer;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tonic::Request;

#[derive(Clone)]
pub struct CooldownManager {
    cooldowns: Arc<FastMap<String, Duration>>,
    map: FastDashMap<(String, &'static str), Instant>,
}

impl CooldownManager {
    pub fn new() -> Self {
        Self {
            cooldowns: Arc::new(
                config::get()
                    .network
                    .method_cooldowns
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.to_string(),
                            utils::serde_duration::deserialize::<
                                StringDeserializer<serde_json::Error>,
                            >(v.clone().into_deserializer())
                            .expect("Failed to deserialize duration"),
                        )
                    })
                    .collect(),
            ),
            map: FastDashMap::with_capacity_and_hasher(1024, Default::default()),
        }
    }

    pub fn throttle<T>(&self, req: &Request<T>, method: &'static str) -> Result<(), Error> {
        let cooldown = match self.cooldowns.get(method) {
            Some(cd) => *cd,
            None => return Ok(()),
        };

        let addr_str = req
            .metadata()
            .get("forwarded-addr")
            .ok_or_else(|| Error::internal("No forwarded address found"))?
            .to_str()
            .map_err(|err| Error::internal(format!("Failed to parse forwarded address: {err}")))?;

        let now = Instant::now();

        let key = (addr_str.to_string(), method);

        match self.map.entry(key) {
            Entry::Occupied(mut entry) => {
                let last_time = entry.get();
                if *last_time + cooldown > now {
                    let remaining = (*last_time + cooldown) - now;
                    return Err(Error::rate_limit(format!(
                        "This function is on cooldown. Try again in {}s.",
                        remaining.as_secs().max(1)
                    )));
                }
                entry.insert(now);
            }
            Entry::Vacant(entry) => {
                entry.insert(now);
            }
        }

        Ok(())
    }

    pub fn maintain(&self) {
        let now = Instant::now();

        self.map.retain(|(_, path), last_time| {
            if let Some(&cooldown) = self.cooldowns.get(*path) {
                *last_time + cooldown > now
            } else {
                tracing::warn!("Method '{path}' registered but not found in cooldowns");
                false
            }
        });
    }
}
