// Common test utilities and fixtures

use clear_urls_bot::config::Config;
use clear_urls_bot::db::Db;

/// Setup test database with in-memory SQLite
pub async fn setup_test_db() -> Db {
    let db = Db::new("sqlite::memory:").await.unwrap();
    db
}

/// Create test configuration
pub fn test_config() -> Config {
    Config {
        bot_token: "test_token".to_string(),
        bot_username: "@test_bot".to_string(),
        admin_id: 12345,
        database_url: "sqlite::memory:".to_string(),
        clearurls_source: "https://rules2.clearurls.xyz/data.minify.json".to_string(),
        webhook_url: None,
        webhook_port: 8443,
        cookie_key: "0123456789abcdef0123456789abcdef".to_string(),
        ai_api_key: None,
        ai_api_base: "https://api.openai.com/v1".to_string(),
        ai_model: "gpt-4".to_string(),
        inline_max_results: 5,
    }
}

/// Sample URLs for testing
pub mod test_urls {
    pub const CLEAN_URL: &str = "https://example.com/page";
    pub const URL_WITH_UTM: &str = "https://example.com/page?utm_source=test&utm_medium=email";
    pub const AMAZON_URL: &str = "https://www.amazon.com/product/dp/B08X6PZTKS?ref_=ast_sto_dp&th=1&psc=1";
    pub const YOUTUBE_URL: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&feature=share";
    pub const MALICIOUS_URL: &str = "http://malware-test.example.com";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_setup() {
        let db = setup_test_db().await;
        // Verify database is initialized
        assert!(db.get_user_config(12345).await.is_ok());
    }

    #[test]
    fn test_config_creation() {
        let config = test_config();
        assert_eq!(config.bot_username, "@test_bot");
        assert_eq!(config.admin_id, 12345);
    }
}
