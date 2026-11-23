//! Security — Input validation module.
//!
//! Validates all user inputs and external data before they enter the
//! application. Prevents injection, overflow, and malformed data attacks.

/// Validation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
        }
    }

    pub fn invalid(error: String) -> Self {
        Self {
            valid: false,
            errors: vec![error],
        }
    }

    pub fn add_error(mut self, error: String) -> Self {
        self.errors.push(error);
        self.valid = false;
        self
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

/// Maximum lengths for various input fields.
pub mod limits {
    pub const MAX_EMAIL_LENGTH: usize = 254;
    pub const MAX_PASSWORD_LENGTH: usize = 128;
    pub const MIN_PASSWORD_LENGTH: usize = 8;
    pub const MAX_NAME_LENGTH: usize = 100;
    pub const MAX_LATITUDE: f64 = 90.0;
    pub const MIN_LATITUDE: f64 = -90.0;
    pub const MAX_LONGITUDE: f64 = 180.0;
    pub const MIN_LONGITUDE: f64 = -180.0;
    pub const MAX_ACCURACY_METERS: f64 = 10_000_000.0;
    pub const MAX_SEARCH_QUERY_LENGTH: usize = 120;
}

/// Email validation.
pub fn validate_email(email: &str) -> ValidationResult {
    if email.is_empty() {
        return ValidationResult::invalid("Email is required".to_string());
    }

    if email.chars().count() > limits::MAX_EMAIL_LENGTH {
        return ValidationResult::invalid("Email is too long (max 254 characters)".to_string());
    }

    if !email.contains('@') {
        return ValidationResult::invalid("Email must contain @".to_string());
    }

    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return ValidationResult::invalid("Email must contain exactly one @".to_string());
    }

    let local = parts[0];
    let domain = parts[1];

    if local.is_empty() || domain.is_empty() {
        return ValidationResult::invalid("Email parts cannot be empty".to_string());
    }

    if !domain.contains('.') {
        return ValidationResult::invalid("Email domain must contain a dot".to_string());
    }

    // Check for suspicious characters
    if contains_display_control(email) {
        return ValidationResult::invalid("Email contains invalid characters".to_string());
    }

    ValidationResult::valid()
}

/// Password validation.
pub fn validate_password(password: &str) -> ValidationResult {
    if password.is_empty() {
        return ValidationResult::invalid("Password is required".to_string());
    }

    if password.chars().count() < limits::MIN_PASSWORD_LENGTH {
        return ValidationResult::invalid(format!(
            "Password must be at least {} characters",
            limits::MIN_PASSWORD_LENGTH
        ));
    }

    if password.chars().count() > limits::MAX_PASSWORD_LENGTH {
        return ValidationResult::invalid("Password is too long (max 128 characters)".to_string());
    }

    // Check for null bytes and control characters
    if password.contains('\0') {
        return ValidationResult::invalid("Password contains null bytes".to_string());
    }

    if password.chars().any(|c| c.is_control()) {
        return ValidationResult::invalid("Password contains control characters".to_string());
    }

    ValidationResult::valid()
}

/// Latitude validation.
pub fn validate_latitude(lat: f64) -> ValidationResult {
    if lat.is_nan() || lat.is_infinite() {
        return ValidationResult::invalid("Latitude must be a valid number".to_string());
    }

    if lat < limits::MIN_LATITUDE || lat > limits::MAX_LATITUDE {
        return ValidationResult::invalid(format!(
            "Latitude must be between {} and {}",
            limits::MIN_LATITUDE,
            limits::MAX_LATITUDE
        ));
    }

    ValidationResult::valid()
}

/// Longitude validation.
pub fn validate_longitude(lon: f64) -> ValidationResult {
    if lon.is_nan() || lon.is_infinite() {
        return ValidationResult::invalid("Longitude must be a valid number".to_string());
    }

    if lon < limits::MIN_LONGITUDE || lon > limits::MAX_LONGITUDE {
        return ValidationResult::invalid(format!(
            "Longitude must be between {} and {}",
            limits::MIN_LONGITUDE,
            limits::MAX_LONGITUDE
        ));
    }

    ValidationResult::valid()
}

/// Accuracy validation.
pub fn validate_accuracy(accuracy: Option<f64>) -> ValidationResult {
    if let Some(acc) = accuracy {
        if acc.is_nan() || acc.is_infinite() {
            return ValidationResult::invalid("Accuracy must be a valid number".to_string());
        }

        if acc < 0.0 {
            return ValidationResult::invalid("Accuracy cannot be negative".to_string());
        }

        if acc > limits::MAX_ACCURACY_METERS {
            return ValidationResult::invalid(format!(
                "Accuracy cannot exceed {} meters",
                limits::MAX_ACCURACY_METERS
            ));
        }
    }

    ValidationResult::valid()
}

/// Search query validation.
pub fn validate_search_query(query: &str) -> ValidationResult {
    if query.is_empty() {
        return ValidationResult::invalid("Search query is required".to_string());
    }

    if query.chars().count() > limits::MAX_SEARCH_QUERY_LENGTH {
        return ValidationResult::invalid(format!(
            "Search query is too long (max {} characters)",
            limits::MAX_SEARCH_QUERY_LENGTH
        ));
    }

    if query.contains('\0') {
        return ValidationResult::invalid("Search query contains null bytes".to_string());
    }

    ValidationResult::valid()
}

/// Combined location validation.
pub fn validate_location(lat: f64, lon: f64, accuracy: Option<f64>) -> ValidationResult {
    let mut result = ValidationResult::valid();
    result = result.and(validate_latitude(lat));
    result = result.and(validate_longitude(lon));
    result = result.and(validate_accuracy(accuracy));
    result
}

/// Helper to combine validation results.
impl ValidationResult {
    pub fn and(mut self, other: ValidationResult) -> ValidationResult {
        if !other.is_valid() {
            self.valid = false;
            self.errors.extend(other.errors);
        }
        self
    }
}

/// Format controls that can reorder or hide displayed text. `char::is_control`
/// only covers C0/C1; the bidi overrides and isolates are `Cf`.
const BIDI_CONTROLS: &[char] = &[
    '\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}',
    '\u{2067}', '\u{2068}', '\u{2069}',
];

/// True when `value` contains a character that can distort how a name, label
/// or note is displayed (C0/C1 controls or Unicode bidi format controls).
pub fn contains_display_control(value: &str) -> bool {
    value
        .chars()
        .any(|c| c.is_control() || BIDI_CONTROLS.contains(&c))
}

/// True when `value` contains a Unicode bidi format control. Unlike
/// [`contains_display_control`], ordinary newlines and tabs are allowed so a
/// multi-line note message can still contain paragraphs.
pub fn contains_bidi_control(value: &str) -> bool {
    value.chars().any(|c| BIDI_CONTROLS.contains(&c))
}

/// Validate a username/display name.
pub fn validate_name(name: &str) -> ValidationResult {
    if name.is_empty() {
        return ValidationResult::invalid("Name is required".to_string());
    }

    if name.chars().count() > limits::MAX_NAME_LENGTH {
        return ValidationResult::invalid(format!(
            "Name is too long (max {} characters)",
            limits::MAX_NAME_LENGTH
        ));
    }

    if contains_display_control(name) {
        return ValidationResult::invalid("Name contains invalid characters".to_string());
    }

    ValidationResult::valid()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_email() {
        assert!(validate_email("alice@example.com").is_valid());
        assert!(validate_email("user+tag@domain.co.uk").is_valid());
        assert!(validate_email("café@example.com").is_valid());
    }

    #[test]
    fn invalid_email() {
        assert!(!validate_email("").is_valid());
        assert!(!validate_email("no-at-sign").is_valid());
        assert!(!validate_email("@domain.com").is_valid());
        assert!(!validate_email("user@").is_valid());
        assert!(!validate_email("user@domain").is_valid());
        assert!(!validate_email("a@b@c.com").is_valid());
        assert!(!validate_email("a\u{202e}@example.com").is_valid());
        assert!(!validate_email("a\tb@example.com").is_valid());
    }

    #[test]
    fn valid_password() {
        assert!(validate_password("password123").is_valid());
        assert!(validate_password("Str0ng!Pass").is_valid());
        assert!(validate_password("éééééééé").is_valid());
    }

    #[test]
    fn invalid_password() {
        assert!(!validate_password("").is_valid());
        assert!(!validate_password("short").is_valid());
        assert!(!validate_password("ééééééé").is_valid());
        assert!(!validate_password(&"a".repeat(129)).is_valid());
    }

    #[test]
    fn valid_latitude() {
        assert!(validate_latitude(0.0).is_valid());
        assert!(validate_latitude(90.0).is_valid());
        assert!(validate_latitude(-90.0).is_valid());
        assert!(validate_latitude(41.8902).is_valid());
    }

    #[test]
    fn invalid_latitude() {
        assert!(!validate_latitude(91.0).is_valid());
        assert!(!validate_latitude(-91.0).is_valid());
        assert!(!validate_latitude(f64::NAN).is_valid());
        assert!(!validate_latitude(f64::INFINITY).is_valid());
    }

    #[test]
    fn valid_longitude() {
        assert!(validate_longitude(0.0).is_valid());
        assert!(validate_longitude(180.0).is_valid());
        assert!(validate_longitude(-180.0).is_valid());
        assert!(validate_longitude(-87.6323).is_valid());
    }

    #[test]
    fn invalid_longitude() {
        assert!(!validate_longitude(181.0).is_valid());
        assert!(!validate_longitude(-181.0).is_valid());
    }

    #[test]
    fn combined_location_validation() {
        assert!(validate_location(41.8902, -87.6323, Some(8.0)).is_valid());
        assert!(!validate_location(200.0, -87.6323, None).is_valid());
        assert!(!validate_location(41.8902, 200.0, None).is_valid());
        assert!(!validate_location(41.8902, -87.6323, Some(-1.0)).is_valid());
    }

    #[test]
    fn search_query_validation() {
        assert!(validate_search_query("café").is_valid());
        assert!(!validate_search_query("").is_valid());
        assert!(validate_search_query(&"é".repeat(120)).is_valid());
        assert!(!validate_search_query(&"é".repeat(121)).is_valid());
    }

    #[test]
    fn name_validation() {
        assert!(validate_name("Alice").is_valid());
        assert!(!validate_name("").is_valid());
        assert!(validate_name(&"é".repeat(60)).is_valid());
        assert!(!validate_name(&"a".repeat(101)).is_valid());
        assert!(!validate_name(&"é".repeat(101)).is_valid());
        assert!(!validate_name("Alice\u{202E}").is_valid());
        assert!(!validate_name("Ali\tce").is_valid());
    }
}
