use crate::config;
use crate::error::Error;
use crate::types::FastDashMap;
use lettre::message::header::ContentType;
use lettre::message::{Mailbox, MessageBuilder, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tera::{Context, Tera};

const EMAIL_CODE_TEMPLATE_NAME: &str = "email_code_tmp";

#[derive(Clone, Debug)]
pub struct EmailRegister {
    register: FastDashMap<String, EmailEntry>,
    trans: AsyncSmtpTransport<Tokio1Executor>,
    tera: Tera,
    token_len: usize,
}

impl EmailRegister {
    pub async fn new() -> Self {
        let config = config::get();

        let pass = std::fs::read_to_string(&config.email.smtp_password)
            .expect("Failed to read SMTP password")
            .trim()
            .to_string();

        let creds = Credentials::new(config.email.smtp_user.clone(), pass);

        let trans = if config.email.relay {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.email.smtp)
                .expect("Failed to build SMTP relay-transport")
        } else {
            if !cfg!(debug_assertions) {
                tracing::warn!("SMTP relay is not enabled. Using unsecure SMTP transport...");
            }

            let (addr, port) = config
                .email
                .smtp
                .split_once(':')
                .expect("Invalid SMTP address");

            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(addr)
                .port(port.parse().expect("Failed to parse SMTP transport port"))
        };

        let mut tera = Tera::new();

        let email_code_tmp = tokio::fs::read_to_string(&config.email.email_code_tmp)
            .await
            .expect("Failed to read email code template");

        tera.add_raw_template(EMAIL_CODE_TEMPLATE_NAME, &email_code_tmp)
            .expect("Failed to add email template");

        Self {
            register: FastDashMap::with_capacity_and_hasher(10, Default::default()),
            trans: trans.credentials(creds).build(),
            tera,
            token_len: config.email.verify_token_len,
        }
    }

    pub async fn register_email(&self, email: String) -> Result<(), Error> {
        let config = config::get();

        let mut token_bytes = vec![0u8; self.token_len];
        for byte in &mut token_bytes {
            *byte = fastrand::u8(b'0'..=b'9');
        }

        let token = unsafe { String::from_utf8_unchecked(token_bytes) };

        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| Error::internal(format!("Failed to get system time: {err}")))?
            .as_secs()
            + config::get().email.exp;

        let addr =
            Address::from_str(&email).map_err(|_| Error::invalid_format("Invalid email format"))?;

        let format_exp = chrono::DateTime::from_timestamp_secs(expires as i64)
            .ok_or(Error::internal("Failed to convert timestamp to DateTime"))?
            .to_rfc3339();

        let (text_body, html_body) = self.build_email_code_template(&token, &format_exp)?;

        let msg = MessageBuilder::new()
            .to(Mailbox::new(None, addr))
            .subject("Your E-Mail Verification Code")
            .from(Mailbox::new(
                None,
                Address::from_str(&config.email.no_reply_mail).expect("Invalid no reply email"),
            ))
            .header(ContentType::TEXT_PLAIN)
            .multipart(MultiPart::alternative_plain_html(text_body, html_body))
            .expect("Failed to build E-Mail");

        self.trans.send(msg).await.expect("Failed to send E-Mail");

        self.register.insert(email, EmailEntry { token, expires });

        Ok(())
    }

    pub fn verify_email(&self, email: &String, token: String) -> Result<(), Error> {
        let entry_token = self
            .get_email_token(email)
            .ok_or(Error::not_found("E-Mail not found"))?;

        if token != entry_token {
            return Err(Error::unauthorized("Invalid token"));
        }

        self.register.remove(email);

        Ok(())
    }

    pub fn get_email_token(&self, email: &String) -> Option<String> {
        self.register.get(email).map(|e| e.token.clone())
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

    pub fn print_status(&self) {
        tracing::info!("Email Register Length: {}", self.register.len());
    }

    fn build_email_code_template(&self, token: &str, exp: &str) -> Result<(String, String), Error> {
        let text = format!(
            "Your verification code is: {}\nThis code will expire in {}.",
            token, exp
        );

        let mut context = Context::new();

        context.insert("token", token);
        context.insert("exp", exp);

        let html = self
            .tera
            .render(EMAIL_CODE_TEMPLATE_NAME, &context)
            .map_err(|err| Error::internal(format!("Failed to render email template: {err}")))?;

        Ok((text, html))
    }
}

#[derive(Clone, Debug)]
struct EmailEntry {
    token: String,
    expires: u64,
}
