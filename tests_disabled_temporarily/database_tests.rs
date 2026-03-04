// Tests for database operations

mod common;

use common::setup_test_db;

#[tokio::test]
async fn test_user_config_creation() {
    let db = setup_test_db().await;
    let user_id = 12345;
    
    // Get or create user config
    let config = db.get_user_config(user_id).await.unwrap();
    assert_eq!(config.user_id, user_id);
    assert_eq!(config.lang, "en"); // Default language
}

#[tokio::test]
async fn test_user_config_update() {
    let db = setup_test_db().await;
    let user_id = 12345;
    
    // Update language
    db.set_user_lang(user_id, "it").await.unwrap();
    
    let config = db.get_user_config(user_id).await.unwrap();
    assert_eq!(config.lang, "it");
}

#[tokio::test]
async fn test_statistics_tracking() {
    let db = setup_test_db().await;
    let user_id = 12345;
    
    // Initial state
    let stats = db.get_user_stats(user_id).await.unwrap();
    assert_eq!(stats.total_links, 0);
    
    // Increment stats
    db.increment_link_count(user_id).await.unwrap();
    db.increment_link_count(user_id).await.unwrap();
    
    let updated_stats = db.get_user_stats(user_id).await.unwrap();
    assert_eq!(updated_stats.total_links, 2);
}

#[tokio::test]
async fn test_history_tracking() {
    let db = setup_test_db().await;
    let user_id = 12345;
    
    let original = "https://example.com?utm_source=test";
    let cleaned = "https://example.com";
    let provider = "RegexRules";
    
    // Add to history
    db.add_to_history(user_id, original, cleaned, provider).await.unwrap();
    
    // Retrieve history
    let history = db.get_history(user_id, 10).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].original_url, original);
    assert_eq!(history[0].cleaned_url, cleaned);
    assert_eq!(history[0].provider, provider);
}

#[tokio::test]
async fn test_whitelist_operations() {
    let db = setup_test_db().await;
    let user_id = 12345;
    let domain = "trusted.example.com";
    
    // Add to whitelist
    db.add_to_whitelist(user_id, domain).await.unwrap();
    
    // Check if whitelisted
    let is_whitelisted = db.is_whitelisted(user_id, domain).await.unwrap();
    assert!(is_whitelisted);
    
    // Get whitelist
    let whitelist = db.get_whitelist(user_id).await.unwrap();
    assert_eq!(whitelist.len(), 1);
    assert_eq!(whitelist[0], domain);
    
    // Remove from whitelist
    db.remove_from_whitelist(user_id, domain).await.unwrap();
    let is_still_whitelisted = db.is_whitelisted(user_id, domain).await.unwrap();
    assert!(!is_still_whitelisted);
}

#[tokio::test]
async fn test_top_users_leaderboard() {
    let db = setup_test_db().await;
    
    // Create multiple users with different link counts
    for user_id in 1..=5 {
        for _ in 0..user_id {
            db.increment_link_count(user_id as i64).await.unwrap();
        }
    }
    
    // Get top users
    let top_users = db.get_top_users(3).await.unwrap();
    assert_eq!(top_users.len(), 3);
    
    // Should be sorted by link count descending
    assert!(top_users[0].total_links >= top_users[1].total_links);
    assert!(top_users[1].total_links >= top_users[2].total_links);
}

#[tokio::test]
async fn test_global_stats() {
    let db = setup_test_db().await;
    
    // Create some activity
    for user_id in 1..=3 {
        db.get_user_config(user_id).await.unwrap();
        db.increment_link_count(user_id).await.unwrap();
    }
    
    let global_stats = db.get_global_stats().await.unwrap();
    assert_eq!(global_stats.total_users, 3);
    assert_eq!(global_stats.total_links, 3);
}

#[tokio::test]
async fn test_feature_flags() {
    let db = setup_test_db().await;
    let user_id = 12345;
    
    // Enable a feature
    db.set_feature_flag(user_id, "ai_engine", true).await.unwrap();
    
    let is_enabled = db.is_feature_enabled(user_id, "ai_engine").await.unwrap();
    assert!(is_enabled);
    
    // Disable feature
    db.set_feature_flag(user_id, "ai_engine", false).await.unwrap();
    let is_still_enabled = db.is_feature_enabled(user_id, "ai_engine").await.unwrap();
    assert!(!is_still_enabled);
}
