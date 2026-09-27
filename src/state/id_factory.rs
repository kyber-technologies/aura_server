use crate::error::Error;
use crate::types::UniqueId;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

#[derive(Clone)]
pub struct IdFactory {
    factory: sonyflake::Sonyflake,
}

impl IdFactory {
    pub fn new(machine_id: u16) -> Self {
        Self {
            factory: sonyflake::Sonyflake::builder()
                .machine_id(&|| Ok(machine_id))
                // NOTE: THIS CANNOT AND MUST NOT CHANGE:
                // Use 01-01-2020 as start time
                .start_time(DateTime::<Utc>::from_naive_utc_and_offset(
                    NaiveDate::from_ymd_opt(2020, 1, 1)
                        .unwrap()
                        .and_time(NaiveTime::MIN),
                    Utc,
                ))
                .finalize()
                .expect("Failed to build id factory"),
        }
    }

    pub fn next_id(&self) -> Result<UniqueId, Error> {
        self.factory
            .next_id()
            .map(|id| id.to_u64() as i64)
            .map_err(|err| Error::internal(format!("Failed to generate ID: {err}")))
    }
}
