use std::collections::HashMap;
use once_cell::sync::Lazy;
use std::sync::Mutex;

static URL_CACHE: Lazy<Mutex<HashMap<String, bool>>> = Lazy::new(|| Mutex::new(HashMap::new()));

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
// ...existing code...
