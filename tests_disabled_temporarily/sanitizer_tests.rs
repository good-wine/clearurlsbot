// Tests for URL sanitization and rule engine

mod common;

use clear_urls_bot::sanitizer::validation::UrlValidator;
use clear_urls_bot::sanitizer::RuleEngine;
use common::test_urls::*;

#[tokio::test]
async fn test_clean_url_unchanged() {
    let rules = RuleEngine::new_lazy("https://rules2.clearurls.xyz/data.minify.json");
    let result = rules.clean_url(CLEAN_URL).await;
    
    assert!(result.is_ok());
    let cleaned = result.unwrap();
    assert_eq!(cleaned, CLEAN_URL);
}

#[tokio::test]
async fn test_utm_parameters_removed() {
    let rules = RuleEngine::new_lazy("https://rules2.clearurls.xyz/data.minify.json");
    let result = rules.clean_url(URL_WITH_UTM).await;
    
    assert!(result.is_ok());
    let cleaned = result.unwrap();
    assert!(!cleaned.contains("utm_source"));
    assert!(!cleaned.contains("utm_medium"));
    assert!(cleaned.contains("example.com/page"));
}

#[tokio::test]
async fn test_amazon_tracking_removed() {
    let rules = RuleEngine::new_lazy("https://rules2.clearurls.xyz/data.minify.json");
    let result = rules.clean_url(AMAZON_URL).await;
    
    assert!(result.is_ok());
    let cleaned = result.unwrap();
    // Should keep product ID but remove tracking params
    assert!(cleaned.contains("B08X6PZTKS"));
    assert!(!cleaned.contains("ref_="));
}

#[tokio::test]
async fn test_youtube_feature_removed() {
    let rules = RuleEngine::new_lazy("https://rules2.clearurls.xyz/data.minify.json");
    let result = rules.clean_url(YOUTUBE_URL).await;
    
    assert!(result.is_ok());
    let cleaned = result.unwrap();
    // Should keep video ID but remove feature param
    assert!(cleaned.contains("dQw4w9WgXcQ"));
    assert!(!cleaned.contains("feature="));
}

#[test]
fn test_url_validator_valid() {
    let validator = UrlValidator::new();
    assert!(validator.is_valid_url("https://example.com"));
    assert!(validator.is_valid_url("http://test.org"));
}

#[test]
fn test_url_validator_invalid() {
    let validator = UrlValidator::new();
    assert!(!validator.is_valid_url("not a url"));
    assert!(!validator.is_valid_url("ftp://unsupported.com"));
    assert!(!validator.is_valid_url("javascript:alert(1)"));
}

#[test]
fn test_url_validator_domain_extraction() {
    let validator = UrlValidator::new();
    
    let domain1 = validator.extract_domain("https://www.example.com/path");
    assert_eq!(domain1, Some("www.example.com".to_string()));
    
    let domain2 = validator.extract_domain("http://sub.domain.org:8080/page");
    assert_eq!(domain2, Some("sub.domain.org".to_string()));
    
    let domain3 = validator.extract_domain("not a url");
    assert_eq!(domain3, None);
}

#[tokio::test]
async fn test_multiple_urls_cleaning() {
    let rules = RuleEngine::new_lazy("https://rules2.clearurls.xyz/data.minify.json");
    
    let urls = vec![CLEAN_URL, URL_WITH_UTM, YOUTUBE_URL];
    let mut cleaned_count = 0;
    
    for url in urls {
        if let Ok(cleaned) = rules.clean_url(url).await {
            if cleaned != url {
                cleaned_count += 1;
            }
        }
    }
    
    // At least 2 URLs should be cleaned (UTM and YouTube)
    assert!(cleaned_count >= 2);
}
