//! Utility functions for the web application

use chrono::{DateTime, Utc};
use leptos::*;
use web_sys::window;

/// Format a date for display
pub fn format_date(date: &DateTime<Utc>) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Format a date and time for display
pub fn format_datetime(date: &DateTime<Utc>) -> String {
    date.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Format a relative time (e.g., "2 hours ago")
pub fn format_relative_time(date: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*date);

    if duration.num_days() > 0 {
        format!("{} days ago", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{} hours ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{} minutes ago", duration.num_minutes())
    } else {
        "Just now".to_string()
    }
}

/// Capitalize the first letter of a string
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Truncate a string to a maximum length
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Format a file size in bytes to human-readable format
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Validate email format
pub fn is_valid_email(email: &str) -> bool {
    let email_regex = regex::Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    email_regex.is_match(email)
}

/// Validate phone number format (basic validation)
pub fn is_valid_phone(phone: &str) -> bool {
    let phone_regex = regex::Regex::new(r"^\+?[\d\s\-\(\)]+$").unwrap();
    phone_regex.is_match(phone) && phone.chars().filter(|c| c.is_ascii_digit()).count() >= 10
}

/// Generate a random ID
pub fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Get the current timestamp
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// Local storage utilities
pub mod local_storage {
    use super::*;

    /// Save data to local storage
    pub fn save<T: serde::Serialize>(key: &str, data: &T) -> Result<(), String> {
        let window = window().ok_or("No window available")?;
        let storage = window
            .local_storage()
            .map_err(|_| "Failed to access local storage")?
            .ok_or("Local storage not available")?;

        let serialized = serde_json::to_string(data)
            .map_err(|e| format!("Failed to serialize data: {}", e))?;

        storage
            .set_item(key, &serialized)
            .map_err(|_| "Failed to save to local storage")?;

        Ok(())
    }

    /// Load data from local storage
    pub fn load<T: serde::de::DeserializeOwned>(key: &str) -> Result<Option<T>, String> {
        let window = window().ok_or("No window available")?;
        let storage = window
            .local_storage()
            .map_err(|_| "Failed to access local storage")?
            .ok_or("Local storage not available")?;

        let serialized = storage
            .get_item(key)
            .map_err(|_| "Failed to load from local storage")?;

        match serialized {
            Some(data) => {
                let deserialized = serde_json::from_str::<T>(&data)
                    .map_err(|e| format!("Failed to deserialize data: {}", e))?;
                Ok(Some(deserialized))
            }
            None => Ok(None),
        }
    }

    /// Remove data from local storage
    pub fn remove(key: &str) -> Result<(), String> {
        let window = window().ok_or("No window available")?;
        let storage = window
            .local_storage()
            .map_err(|_| "Failed to access local storage")?
            .ok_or("Local storage not available")?;

        storage
            .remove_item(key)
            .map_err(|_| "Failed to remove from local storage")?;

        Ok(())
    }

    /// Clear all data from local storage
    pub fn clear() -> Result<(), String> {
        let window = window().ok_or("No window available")?;
        let storage = window
            .local_storage()
            .map_err(|_| "Failed to access local storage")?
            .ok_or("Local storage not available")?;

        storage
            .clear()
            .map_err(|_| "Failed to clear local storage")?;

        Ok(())
    }
}

/// URL utilities
pub mod url {
    use super::*;

    /// Get the current URL
    pub fn current_url() -> Result<String, String> {
        let window = window().ok_or("No window available")?;
        let location = window.location();
        location.href().map_err(|_| "Failed to get current URL".to_string())
    }

    /// Navigate to a URL
    pub fn navigate_to(url: &str) -> Result<(), String> {
        let window = window().ok_or("No window available")?;
        let location = window.location();
        location.set_href(url).map_err(|_| "Failed to navigate".to_string())
    }

    /// Reload the current page
    pub fn reload() -> Result<(), String> {
        let window = window().ok_or("No window available")?;
        let location = window.location();
        location.reload().map_err(|_| "Failed to reload".to_string())
    }

    /// Get query parameters from URL
    pub fn get_query_params() -> Result<std::collections::HashMap<String, String>, String> {
        let window = window().ok_or("No window available")?;
        let location = window.location();
        let search = location.search().map_err(|_| "Failed to get search params".to_string())?;
        
        let mut params = std::collections::HashMap::new();
        if !search.is_empty() && search.starts_with('?') {
            for param in search[1..].split('&') {
                if let Some((key, value)) = param.split_once('=') {
                    params.insert(
                        urlencoding::decode(key).map_err(|_| "Failed to decode key".to_string())?.to_string(),
                        urlencoding::decode(value).map_err(|_| "Failed to decode value".to_string())?.to_string(),
                    );
                }
            }
        }
        
        Ok(params)
    }
}

/// CSS class utilities
pub mod css {
    /// Conditionally join CSS classes
    pub fn classes(classes: &[(&str, bool)]) -> String {
        classes
            .iter()
            .filter_map(|(class, condition)| {
                if *condition {
                    Some(*class)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Join CSS classes
    pub fn join(classes: &[&str]) -> String {
        classes.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize("HELLO"), "HELLO");
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("h"), "H");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello world", 10), "hello world");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("hello", 3), "hello");
        assert_eq!(truncate("hello", 2), "");
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1048576), "1.0 MB");
        assert_eq!(format_file_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_is_valid_email() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("user+tag@domain.co.uk"));
        assert!(!is_valid_email("invalid.email"));
        assert!(!is_valid_email("@domain.com"));
        assert!(!is_valid_email("user@"));
    }

    #[test]
    fn test_is_valid_phone() {
        assert!(is_valid_phone("+1-555-123-4567"));
        assert!(is_valid_phone("(555) 123-4567"));
        assert!(is_valid_phone("5551234567"));
        assert!(!is_valid_phone("123"));
        assert!(!is_valid_phone("abc-def-ghij"));
    }

    #[test]
    fn test_format_date() {
        let date = DateTime::parse_from_rfc3339("2023-01-15T10:30:00Z").unwrap().with_timezone(&Utc);
        assert_eq!(format_date(&date), "2023-01-15");
    }

    #[test]
    fn test_format_datetime() {
        let date = DateTime::parse_from_rfc3339("2023-01-15T10:30:00Z").unwrap().with_timezone(&Utc);
        assert_eq!(format_datetime(&date), "2023-01-15 10:30:00 UTC");
    }

    #[test]
    fn test_css_classes() {
        assert_eq!(css::classes(&[("class1", true), ("class2", false), ("class3", true)]), "class1 class3");
        assert_eq!(css::classes(&[("class1", false), ("class2", false)]), "");
        assert_eq!(css::classes(&[]), "");
    }

    #[test]
    fn test_css_join() {
        assert_eq!(css::join(&["class1", "class2", "class3"]), "class1 class2 class3");
        assert_eq!(css::join(&[]), "");
    }
} 