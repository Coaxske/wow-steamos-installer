//! Build-time boundary for optional testing and addon-manager automation.
use crate::Result;
pub(crate) const UNAVAILABLE: &str =
    "Developer testing tools are not included in this public build. Use normal setup and the manual pointer guide.";
pub(crate) fn check_action(action: &str) -> Result<()> {
    if !cfg!(any(test, feature = "developer-tools"))
        && (action.starts_with("device_")
            || action.starts_with("live_")
            || action.starts_with("controller_")
            || action.starts_with("addon_registration_")
            || action.starts_with("addon_lifecycle_")
            || [
                "review_addon",
                "prepare_gear",
                "download_addon",
                "mark_tested",
            ]
            .contains(&action))
    {
        return Err(UNAVAILABLE.into());
    }
    Ok(())
}
