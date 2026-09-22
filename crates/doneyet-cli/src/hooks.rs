use doneyet_core::model::Conclusion;
use std::process::Command;

pub fn selected<'a>(
    success: Option<&'a str>,
    failure: Option<&'a str>,
    conclusion: &Conclusion,
) -> Option<&'a str> {
    if conclusion.exit_code() == 0 {
        success
    } else {
        failure
    }
}

pub fn run(success: Option<&str>, failure: Option<&str>, conclusion: &Conclusion) {
    let Some(cmd) = selected(success, failure, conclusion) else {
        return;
    };
    match Command::new("sh").arg("-c").arg(cmd).status() {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("doneyet: hook failed ({cmd}: {status})"),
        Err(error) => eprintln!("doneyet: hook failed ({cmd}: {error})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_conclusion_picks_success_hook_only() {
        let picked = selected(Some("s"), Some("f"), &Conclusion::Success);
        assert_eq!(picked, Some("s"));
    }

    #[test]
    fn pass_like_conclusions_pick_success_hook() {
        for conclusion in [
            Conclusion::Success,
            Conclusion::Skipped,
            Conclusion::Neutral,
        ] {
            assert_eq!(
                selected(Some("s"), Some("f"), &conclusion),
                Some("s"),
                "{conclusion:?}"
            );
        }
    }

    #[test]
    fn non_pass_conclusions_pick_failure_hook() {
        for conclusion in [
            Conclusion::Failure,
            Conclusion::TimedOut,
            Conclusion::Cancelled,
            Conclusion::StartupFailure,
            Conclusion::ActionRequired,
            Conclusion::Stale,
            Conclusion::Other("weird".to_string()),
        ] {
            assert_eq!(
                selected(Some("s"), Some("f"), &conclusion),
                Some("f"),
                "{conclusion:?}"
            );
        }
    }

    #[test]
    fn both_hooks_set_returns_exactly_the_matching_one() {
        assert_eq!(
            selected(Some("s"), Some("f"), &Conclusion::Success),
            Some("s")
        );
        assert_eq!(
            selected(Some("s"), Some("f"), &Conclusion::Failure),
            Some("f")
        );
    }

    #[test]
    fn neither_hook_set_returns_none() {
        assert_eq!(selected(None, None, &Conclusion::Success), None);
        assert_eq!(selected(None, None, &Conclusion::Failure), None);
    }

    #[test]
    fn only_matching_hook_set_returns_it() {
        assert_eq!(selected(Some("s"), None, &Conclusion::Success), Some("s"));
        assert_eq!(selected(Some("s"), None, &Conclusion::Failure), None);
        assert_eq!(selected(None, Some("f"), &Conclusion::Success), None);
        assert_eq!(selected(None, Some("f"), &Conclusion::Failure), Some("f"));
    }

    #[test]
    fn run_with_failing_hook_does_not_panic() {
        run(Some("false"), Some("false"), &Conclusion::Success);
        run(
            Some("definitely-not-a-real-command-xyz"),
            None,
            &Conclusion::Success,
        );
        run(None, Some("false"), &Conclusion::Failure);
    }
}
