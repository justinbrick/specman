#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandleScope {
    SpecOnly,
    ImplOnly,
}

pub(crate) fn prompt_scope(prompt_name: &str, argument_name: &str) -> Option<HandleScope> {
    match (prompt_name, argument_name) {
        ("revision", "target") => Some(HandleScope::SpecOnly),
        ("migration", "target") => Some(HandleScope::SpecOnly),
        ("impl", "spec") => Some(HandleScope::SpecOnly),
        ("feat", "target") => Some(HandleScope::ImplOnly),
        ("ref", "target") => Some(HandleScope::ImplOnly),
        ("fix", "target") => Some(HandleScope::ImplOnly),
        ("compliance", "implementation") => Some(HandleScope::ImplOnly),
        _ => None,
    }
}

pub(crate) fn resource_scope_for_artifact(uri_template: &str) -> Option<HandleScope> {
    if uri_template.starts_with("spec://{artifact}") {
        Some(HandleScope::SpecOnly)
    } else if uri_template.starts_with("impl://{artifact}") {
        Some(HandleScope::ImplOnly)
    } else {
        None
    }
}

pub(crate) fn is_constraint_id_surface(uri_template: &str, argument_name: &str) -> bool {
    uri_template == "spec://{artifact}/constraints/{constraint_id}"
        && argument_name == "constraint_id"
}
