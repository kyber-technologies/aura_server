use diesel::{Insertable, Queryable, Selectable};

use crate::schema::{channel_members, channels};
use crate::types::chat::ChannelPermission;

#[derive(Debug)]
pub struct ChannelData {
    pub channel: Channel,
    pub members: Vec<ChannelMember>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = channels)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Channel {
    pub channel_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = channel_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChannelMember {
    pub channel_id: String,
    pub user_id: String,
    pub permission: ChannelPermission,
}
