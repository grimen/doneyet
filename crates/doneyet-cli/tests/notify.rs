use doneyet_cli::notify::message;
use doneyet_core::model::Conclusion;

#[test]
fn success_wording_is_friendly() {
    let (summary, body) = message(&Conclusion::Success);
    assert_eq!(summary, "doneyet");
    assert_eq!(body, "CI run succeeded");
}

#[test]
fn failure_family_says_failed() {
    for conclusion in [Conclusion::Failure, Conclusion::StartupFailure] {
        let (_, body) = message(&conclusion);
        assert_eq!(body, "CI run failed");
    }
}

#[test]
fn every_conclusion_produces_a_body() {
    for conclusion in [
        Conclusion::Success,
        Conclusion::Failure,
        Conclusion::Cancelled,
        Conclusion::Skipped,
        Conclusion::TimedOut,
        Conclusion::StartupFailure,
        Conclusion::ActionRequired,
        Conclusion::Neutral,
        Conclusion::Stale,
        Conclusion::Other("weird".to_string()),
    ] {
        let (_, body) = message(&conclusion);
        assert!(!body.is_empty(), "{conclusion:?}");
    }
}
