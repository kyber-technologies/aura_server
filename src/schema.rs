// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "channel_permission"))]
    pub struct ChannelPermission;

    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "user_role"))]
    pub struct UserRole;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ChannelPermission;

    channel_members (channel_id, user_id) {
        channel_id -> Text,
        user_id -> Text,
        permission -> ChannelPermission,
    }
}

diesel::table! {
    channels (channel_id) {
        channel_id -> Text,
        name -> Text,
        description -> Text,
    }
}

diesel::table! {
    messages (message_id) {
        message_id -> Text,
        channel_id -> Text,
        user_id -> Text,
        content -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    resources (namespace, key) {
        namespace -> Text,
        key -> Text,
        meta -> Jsonb,
        user_id -> Text,
    }
}

diesel::table! {
    user_blocks (user_id, blocked_user_id) {
        user_id -> Text,
        blocked_user_id -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::UserRole;

    users (user_id) {
        user_id -> Text,
        username -> Text,
        email -> Text,
        password -> Text,
        role -> UserRole,
        icon -> Jsonb,
        notifications -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::joinable!(channel_members -> channels (channel_id));
diesel::joinable!(channel_members -> users (user_id));
diesel::joinable!(messages -> channels (channel_id));
diesel::joinable!(messages -> users (user_id));
diesel::joinable!(resources -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    channel_members,
    channels,
    messages,
    resources,
    user_blocks,
    users,
);
