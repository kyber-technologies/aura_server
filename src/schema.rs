// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "channel_permission"))]
    pub struct ChannelPermission;

    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "post_reaction"))]
    pub struct PostReaction;

    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "resource_namespace_type"))]
    pub struct ResourceNamespaceType;

    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "user_role"))]
    pub struct UserRole;
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::sql_types::ChannelPermission;

    channel_members (channel_id, user_id) {
        channel_id -> Text,
        user_id -> Text,
        permission -> ChannelPermission,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    channels (channel_id) {
        channel_id -> Text,
        name -> Text,
        description -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    messages (message_id) {
        message_id -> Text,
        channel_id -> Text,
        user_id -> Text,
        content -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::sql_types::PostReaction;

    post_reactions (post_id, user_id) {
        post_id -> Text,
        user_id -> Text,
        reaction -> PostReaction,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    posts (post_id) {
        post_id -> Text,
        author_id -> Text,
        content -> Jsonb,
        timestamp -> Timestamptz,
        parent_id -> Nullable<Text>,
        embedding -> Nullable<Vector>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
    use super::sql_types::ResourceNamespaceType;

    resources (namespace_type, namespace_id, key) {
        namespace_type -> ResourceNamespaceType,
        namespace_id -> Text,
        key -> Text,
        meta -> Jsonb,
        user_id -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    user_blocks (user_id, blocked_user_id) {
        user_id -> Text,
        blocked_user_id -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;

    user_follows (follower_id, followed_id) {
        follower_id -> Text,
        followed_id -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use pgvector::sql_types::*;
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
        embedding -> Nullable<Vector>,
    }
}

diesel::joinable!(channel_members -> channels (channel_id));
diesel::joinable!(channel_members -> users (user_id));
diesel::joinable!(messages -> channels (channel_id));
diesel::joinable!(messages -> users (user_id));
diesel::joinable!(post_reactions -> posts (post_id));
diesel::joinable!(post_reactions -> users (user_id));
diesel::joinable!(posts -> users (author_id));
diesel::joinable!(resources -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    channel_members,
    channels,
    messages,
    post_reactions,
    posts,
    resources,
    user_blocks,
    user_follows,
    users,
);
