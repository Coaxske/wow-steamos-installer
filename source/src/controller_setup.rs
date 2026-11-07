// Automatic Steam pointer editing is unavailable in this release.
// Existing reviews, jobs, permissions, results and backups stay untouched.
const CONTROLLER_AUTOMATION_UNAVAILABLE: &str =
    "Automatic pointer configuration is unavailable in this version. Use Gaming Mode pointer setup for manual instructions.";

fn controller_review(_ctx: &Context, _req: &Json) -> Result<Json> {
    Err(CONTROLLER_AUTOMATION_UNAVAILABLE.into())
}

fn controller_apply(_ctx: &Context, _req: &Json) -> Result<Json> {
    Err(CONTROLLER_AUTOMATION_UNAVAILABLE.into())
}

pub(crate) fn validate_controller_target(_req: Json) -> Result<()> {
    Err(CONTROLLER_AUTOMATION_UNAVAILABLE.into())
}
