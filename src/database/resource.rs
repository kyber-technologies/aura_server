use diesel::{Insertable, Queryable, Selectable};
use serde_json::Value;

use crate::schema::resources;

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = resources)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ResourceDescriptor {
    pub namespace: String,
    pub key: String,
    pub meta: Value,
    pub user_id: String,
}
