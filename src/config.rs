use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::env;

const DEFAULT_DATABASE_URL: &str = "sqlite:bot.db";
const DEFAULT_PORT: &str = "3000";
const DEFAULT_CLEARURLS_SOURCE: &str = "https://raw.githubusercontent.com/ClearURLs/Rules/refs/heads/master/data.min.json";
const DEFAULT_AI_API_BASE: &str = "https://api.openai.com/v1";
const DEFAULT_AI_MODEL: &str = "gpt-3.5-turbo";

/// Configuration for the bot, loaded from environment variables.
#[derive(Clone, Debug)]
pub struct Config {
    pub bot_token: String,
    pub bot_username: String,
    pub database_url: String,
    pub server_addr: String,
    pub admin_id: i64,
    pub clearurls_source: String,
    pub ai_api_key: Option<String>,
    pub ai_api_base: String,
    pub ai_model: String,
}

impl Config {
    /// Loads configuration from environment variables.
    ///
    /// # Errors
    /// Returns an error if required environment variables are missing.
    pub fn from_env() -> Result<Self> {
        dotenv().ok();

        let bot_token = env::var("TELOXIDE_TOKEN").context("TELOXIDE_TOKEN deve essere impostato")?;
        let mut bot_username = env::var("BOT_USERNAME").context("BOT_USERNAME deve essere impostato")?;
        if bot_username.starts_with('@') {
            bot_username = bot_username[1..].to_string();
        }
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
        let port = env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
        let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| format!("0.0.0.0:{}", port));

        let admin_id = env::var("ADMIN_ID")
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .unwrap_or(0);

        let clearurls_source = env::var("CLEARURLS_SOURCE").unwrap_or_else(|_| DEFAULT_CLEARURLS_SOURCE.to_string());

        let ai_api_key = env::var("AI_API_KEY").ok().filter(|s| !s.is_empty());
        let ai_api_base = env::var("AI_API_BASE").unwrap_or_else(|_| DEFAULT_AI_API_BASE.to_string());
        let ai_model = env::var("AI_MODEL").unwrap_or_else(|_| DEFAULT_AI_MODEL.to_string());

        Ok(Self {
            bot_token,
            bot_username,
            database_url,
            server_addr,
            admin_id,
            clearurls_source,
            ai_api_key,
            ai_api_base,
            ai_model,
        })
    }

    /// Validates the configuration.
    ///
    /// # Errors
    /// Returns an error if validation fails.
    pub fn validate(&self) -> Result<()> {
        if self.bot_token.is_empty() || !self.bot_token.contains(':') {
            anyhow::bail!("FATAL: TELOXIDE_TOKEN non è valido o è vuoto. Controlla il file .env");
        }
        if self.bot_username.is_empty() {
            anyhow::bail!("FATAL: BOT_USERNAME deve essere configurato");
        }

        // Render Reserved Ports check
        let reserved_ports = ["18012", "18013", "19099"];
        for port in reserved_ports {
            if self.server_addr.contains(port) {
                anyhow::bail!(
                    "FATAL: La porta {} e' riservata da Render e non puo' essere usata.",
                    port
                );
            }
        }
        Ok(())
    }
}
