use crate::config;
use crate::database::DatabaseConnection;
use crate::error::Error;
use crate::logic::user;
use crate::types::common::Auth;
use crate::types::user::User;
use argon2::password_hash::phc::Salt;
use argon2::{Argon2, Params, PasswordHasher, PasswordVerifier, Version};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use std::sync::OnceLock;
use std::time::SystemTime;
use tonic::Request;

const ARGON2_HASH_LEN: usize = 32;

static ARGON2: OnceLock<Argon2> = OnceLock::new();
static KEYS: OnceLock<(EncodingKey, DecodingKey)> = OnceLock::new();

pub async fn init() {
    let config = config::get();

    ARGON2
        .set(Argon2::new(
            argon2::Algorithm::Argon2id,
            Version::V0x13,
            Params::new(19 * 1024, 2, 1, Some(ARGON2_HASH_LEN))
                .expect("Failed to create Argon2 params"),
        ))
        .expect("Failed to initialize Argon2");

    let private = tokio::fs::read(config.service.private_key.as_str())
        .await
        .expect("Failed to read private EdDSA key");

    let public = tokio::fs::read(config.service.public_key.as_str())
        .await
        .expect("Failed to read public EdDSA key");

    KEYS.set((
        EncodingKey::from_ed_pem(&private).expect("Failed to create argon2 encoding key"),
        DecodingKey::from_ed_pem(&public).expect("Failed to create argon2 decoding key"),
    ))
    .expect("Failed to initialize argon2 keys");
}

pub async fn verify<T>(
    database: &mut DatabaseConnection,
    req: &Request<T>,
) -> Result<(User, String), Error> {
    let meta = req.metadata();

    let (_, key) = keys();

    if let Some(token) = meta.get("Authorization") {
        let token = String::from_utf8_lossy(token.as_bytes());
        let claim =
            jsonwebtoken::decode::<Auth>(token.as_bytes(), key, &Validation::new(Algorithm::EdDSA))
                .map_err(|err| match err.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        Error::unauthorized("Token expired")
                    }

                    _ => Error::unauthorized("Invalid token"),
                })?;

        user::get(database, &[claim.claims.user_id])
            .await?
            .into_iter()
            .next()
            .map(|(_, user)| (user, token.to_string()))
            .ok_or(Error::not_found("User not found"))
    } else {
        Err(Error::unauthorized("Missing token"))
    }
}

pub async fn auth(
    database: &mut DatabaseConnection,
    user_id: String,
    password: String,
) -> Result<(String, User), Error> {
    let user = user::get(database, std::slice::from_ref(&user_id))
        .await?
        .into_iter()
        .next();
    let auth = Auth {
        user_id,
        exp: SystemTime::UNIX_EPOCH
            .elapsed()
            .expect("Failed to get current time")
            .as_secs()
            + (config::get().service.token_expiration * 3600),
    };

    let (key, _) = keys();

    if let Some((_, user)) = user {
        if verify_hash(password, user.password.clone()) {
            jsonwebtoken::encode(&Header::new(Algorithm::EdDSA), &auth, key)
                .map(|token| (token, user))
                .map_err(|_| Error::internal("Failed to encode token"))
        } else {
            Err(Error::unauthorized("Invalid credentials"))
        }
    } else {
        Err(Error::unauthorized("Invalid credentials"))
    }
}

pub fn hash(pass: String) -> Result<String, Error> {
    let argon2 = argon2();

    let salt = Salt::generate();

    let hash = argon2
        .hash_password_with_salt(pass.as_bytes(), &salt)
        .map_err(|err| Error::internal(format!("Failed to hash password: {err}")))?;

    Ok(hash.to_string())
}

fn verify_hash(pass: String, hash: String) -> bool {
    argon2()
        .verify_password(pass.as_bytes(), hash.as_str())
        .is_ok()
}

fn argon2<'a>() -> &'a Argon2<'a> {
    ARGON2.get().expect("Argon2 not initialized yet")
}

fn keys<'a>() -> &'a (EncodingKey, DecodingKey) {
    KEYS.get().expect("Keys not initialized yet")
}
