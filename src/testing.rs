use crate::logic::resource;
use crate::types::common::Timestamp;
use crate::types::user::{Notifications, User, UserRole};

pub fn test_user(hash: bool) -> User {
    User {
        user_id: "user".to_string(),
        username: "User".to_string(),
        email: "user@aura.testing".to_string(),
        password: if hash {
            crate::auth::hash("user123".to_string()).expect("Failed to hash moderator password")
        } else {
            "user123".to_string()
        },
        role: UserRole::User,
        created_at: Timestamp::now(),
        icon: resource::build_user_avatar_id("user"),
        notifications: Notifications(Vec::new()),
        channels: Vec::new(),
    }
}

pub fn moderator_user(hash: bool) -> User {
    User {
        user_id: "moderator".to_string(),
        username: "Moderator".to_string(),
        email: "moderator@aura.testing".to_string(),
        password: if hash {
            crate::auth::hash("mod123".to_string()).expect("Failed to hash moderator password")
        } else {
            "mod123".to_string()
        },
        role: UserRole::Moderator,
        created_at: Timestamp::now(),
        icon: resource::build_user_avatar_id("moderator"),
        notifications: Notifications(Vec::new()),
        channels: Vec::new(),
    }
}

pub fn admin_user(hash: bool) -> User {
    User {
        user_id: "admin".to_string(),
        username: "Admin".to_string(),
        email: "admin@aura.testing".to_string(),
        password: if hash {
            crate::auth::hash("admin123".to_string()).expect("Failed to hash moderator password")
        } else {
            "admin123".to_string()
        },
        role: UserRole::Admin,
        created_at: Timestamp::now(),
        icon: resource::build_user_avatar_id("admin"),
        notifications: Notifications(Vec::new()),
        channels: Vec::new(),
    }
}
