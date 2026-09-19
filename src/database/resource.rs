use diesel::{Insertable, Queryable, Selectable};
use diesel_derive_enum::DbEnum;
use serde_json::Value;

use crate::schema::resources;

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = resources)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ResourceDescriptor {
    pub namespace_type: ResourceNamespaceType,
    pub namespace_id: String,
    pub key: String,
    pub meta: Value,
    pub user_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, DbEnum)]
#[db_enum(existing_type_path = "crate::schema::sql_types::ResourceNamespaceType")]
pub enum ResourceNamespaceType {
    #[db_enum(rename = "aura")]
    Aura,
    #[db_enum(rename = "user_icon")]
    UserIcon,
    #[db_enum(rename = "channel")]
    Channel,
}
