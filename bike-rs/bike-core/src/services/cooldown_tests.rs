use super::*;

#[test]
fn unit_happy_report_generation_policy_targets_the_rider() {
    let policy = CooldownType::ReportGeneration;
    assert_eq!(CooldownService::subject_type(Some(7)), "user");
    assert_eq!(policy.action(), "training_report_generation");
    assert_eq!(policy.strategy(), CooldownBackoffStrategy::Simple);
    assert_eq!(policy.duration_seconds(), 30);
    assert!(policy.message(30).contains("Try again in 30 seconds."));
}
