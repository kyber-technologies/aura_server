use crate::config;
use crate::error::Error;
use aura_rust::common::v1::ErrorCode;
use dashmap::DashMap;
use lettre::message::header::ContentType;
use lettre::message::{Mailbox, MessageBuilder};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, SmtpTransport, Transport};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

const TOKEN_LEN: usize = 6;

#[derive(Clone, Debug)]
pub struct EmailRegister {
    register: DashMap<String, EmailEntry>,
    trans: SmtpTransport,
}

impl EmailRegister {
    pub fn new() -> Self {
        let config = config::get();

        let mut pass = std::fs::read_to_string(&config.email_smtp_password)
            .expect("Failed to read SMTP password");

        pass = pass.trim().to_string();

        let creds = Credentials::new(config.email_smtp_user.clone(), pass.to_string());

        Self {
            register: DashMap::with_capacity(10),
            // TODO: make this configurable
            trans: SmtpTransport::from_url(&config.email_smtp)
                .expect("Failed to build SMTP Transport")
                .credentials(creds)
                .build(),
        }
    }

    pub fn register_email(&self, email: String) -> Result<(), Error> {
        let config = config::get();
        let mut token = String::with_capacity(TOKEN_LEN);

        for _ in 0..TOKEN_LEN {
            // TODO: make this faster
            token.push_str(&fastrand::u8(0..=9).to_string());
        }

        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get current time")
            .as_secs()
            + config::get().email_exp;

        let addr = Address::from_str(&email)
            .map_err(|_| Error::new(ErrorCode::InvalidFormat, "Invalid email format"))?;

        let msg = MessageBuilder::new()
            .to(Mailbox::new(None, addr))
            .subject("Your E-Mail Verification Code")
            .from(Mailbox::new(
                None,
                Address::from_str(&config.email_no_reply_mail).expect("Invalid no reply email"),
            ))
            .header(ContentType::TEXT_PLAIN)
            // TODO: make this configurable
            .body(format!("Your E-Mail verification code: {token}"))
            .expect("Failed to build E-Mail");

        self.trans.send(&msg).expect("Failed to send E-Mail");

        self.register.insert(email, EmailEntry { token, expires });

        Ok(())
    }

    pub fn verify_email(&self, email: &String, token: String) -> Result<(), Error> {
        let entry = self
            .register
            .get(email)
            .ok_or(Error::new(ErrorCode::NotFound, "E-Mail not found"))?;

        if token != entry.token {
            return Err(Error::new(ErrorCode::Unauthorized, "Invalid token"));
        }

        self.register.remove(email);

        Ok(())
    }

    pub fn maintain(&self) {
        for entry in &self.register {
            if entry.expires
                <= SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Failed to get current time")
                    .as_secs()
            {
                self.register.remove(entry.key());
            }
        }

        self.register.shrink_to_fit();
    }
}

#[derive(Clone, Debug)]
struct EmailEntry {
    token: String,
    expires: u64,
}
