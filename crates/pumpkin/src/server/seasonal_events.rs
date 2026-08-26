use pumpkin_config::AdvancedConfiguration;
use rand::{rng, seq::SliceRandom};
use time::{Month, OffsetDateTime};

// In fact Mojang also has some Seasonal Events, so we can use that later to match Vanilla :D

#[must_use]
pub fn is_april() -> bool {
    let data = OffsetDateTime::now_utc();
    data.day() == 1 && data.month() == Month::April
}

#[must_use]
pub fn is_halloween() -> bool {
    let data = OffsetDateTime::now_utc();
    data.month() == Month::October && data.day() == 31
}

#[must_use]
pub fn is_christmas() -> bool {
    let data = OffsetDateTime::now_utc();
    data.month() == Month::December && (data.day() >= 24 && data.day() <= 26)
}

#[must_use]
pub fn modify_chat_message(
    message: &str,
    advanced_config: &AdvancedConfiguration,
) -> Option<String> {
    if !advanced_config.fun.april_fools || !is_april() {
        return None;
    }
    let mut words: Vec<&str> = message.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let mut rng = rng();
    words.shuffle(&mut rng);
    let result = words.join(" ");
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasonal_event_dates_parity() {
        // Validation of monthly logic
        assert!(!is_april() || OffsetDateTime::now_utc().month() == Month::April);
        assert!(!is_halloween() || OffsetDateTime::now_utc().month() == Month::October);
        assert!(!is_christmas() || OffsetDateTime::now_utc().month() == Month::December);
    }
}
