pub mod chat;
pub mod general;
pub mod resource;
pub mod user;

pub type GeneralService = general::Service;
pub type ChatService = chat::Service;
pub type ResourceService = resource::Service;
pub type UserService = user::Service;
