use std::collections::HashMap;
use std::sync::Mutex;

static URL_CACHE: Mutex<HashMap<String, bool>> = Mutex::new(HashMap::new());

pub fn is_valid_url(url: &str) -> bool {
    let mut cache = URL_CACHE.lock().unwrap();
    if let Some(&result) = cache.get(url) {
        return result;
    }
    let result = url.starts_with("http://") || url.starts_with("https://");
    cache.insert(url.to_string(), result);
    result
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_url() {
        assert!(is_valid_url("https://example.com"));
        assert!(is_valid_url("http://example.com"));
        assert!(!is_valid_url("ftp://example.com"));
        assert!(!is_valid_url("example.com"));
    }
}
/// Funzioni di validazione input/output
pub fn is_valid_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}
