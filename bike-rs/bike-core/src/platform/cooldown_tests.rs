use super::*;

#[test]
fn unit_happy_backoff_doubles_until_the_configured_cap() {
    assert_eq!(CooldownService::calculate_exponential_backoff(30, 0, 3), 30);
    assert_eq!(
        CooldownService::calculate_exponential_backoff(30, 2, 3),
        120
    );
    assert_eq!(
        CooldownService::calculate_exponential_backoff(30, 4, 3),
        240
    );
}
