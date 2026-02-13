use super::profiles::ProverBackendProfile;
use crate::cli::NewAllocatorArg;
use crate::error::{CliError, Result};
use airbender_build::DEFAULT_GUEST_TOOLCHAIN;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const GITIGNORE_TEMPLATE: &str = include_str!("../../../templates/.gitignore.template");
const ROOT_README_TEMPLATE: &str = include_str!("../../../templates/README.md.template");
const GUEST_CARGO_TEMPLATE: &str = include_str!("../../../templates/guest/Cargo.toml.template");
const GUEST_MAIN_TEMPLATE: &str = include_str!("../../../templates/guest/src/main.rs.template");
const GUEST_TOOLCHAIN_TEMPLATE: &str =
    include_str!("../../../templates/guest/rust-toolchain.toml.template");
const GUEST_CARGO_CONFIG_TEMPLATE: &str =
    include_str!("../../../templates/guest/.cargo/config.toml.template");
const HOST_CARGO_TEMPLATE: &str = include_str!("../../../templates/host/Cargo.toml.template");
const HOST_TOOLCHAIN_TEMPLATE: &str =
    include_str!("../../../templates/host/rust-toolchain.toml.template");
const CUSTOM_ALLOCATOR_MODULE_TEMPLATE: &str =
    include_str!("../../../templates/snippets/custom_allocator_module.rs.template");

#[derive(Clone, Copy)]
struct TemplateFile<'a> {
    relative_path: &'static str,
    source: &'a str,
}

pub(super) struct TemplateContext<'a> {
    project_name: &'a str,
    sdk_dependency: &'a str,
    host_dependency: &'a str,
    enable_std: bool,
    allocator: NewAllocatorArg,
    host_dependency_features: &'a str,
    readme_prover_backend_doc: &'a str,
}

impl<'a> TemplateContext<'a> {
    pub(super) fn new(
        project_name: &'a str,
        sdk_dependency: &'a str,
        host_dependency: &'a str,
        enable_std: bool,
        allocator: NewAllocatorArg,
        host_dependency_features: &'a str,
        readme_prover_backend_doc: &'a str,
    ) -> Self {
        Self {
            project_name,
            sdk_dependency,
            host_dependency,
            enable_std,
            allocator,
            host_dependency_features,
            readme_prover_backend_doc,
        }
    }

    fn replacements(self) -> [(&'static str, String); 11] {
        [
            ("__AIRBENDER_PROJECT_NAME__", self.project_name.to_string()),
            ("__AIRBENDER_SDK_DEP__", self.sdk_dependency.to_string()),
            (
                "__AIRBENDER_SDK_DEFAULT_FEATURES__",
                sdk_default_features(self.allocator).to_string(),
            ),
            (
                "__AIRBENDER_SDK_FEATURES__",
                sdk_features(self.enable_std, self.allocator),
            ),
            ("__AIRBENDER_HOST_DEP__", self.host_dependency.to_string()),
            (
                "__AIRBENDER_HOST_DEP_FEATURES__",
                self.host_dependency_features.to_string(),
            ),
            (
                "__AIRBENDER_PROVER_BACKEND_DOC__",
                self.readme_prover_backend_doc.to_string(),
            ),
            (
                "__AIRBENDER_GUEST_ATTRIBUTES__",
                guest_attributes(self.enable_std).to_string(),
            ),
            (
                "__AIRBENDER_MAIN_ATTR_ARGS__",
                main_attr_args(self.allocator).to_string(),
            ),
            (
                "__AIRBENDER_CUSTOM_ALLOCATOR_MODULE__",
                custom_allocator_module(self.allocator).to_string(),
            ),
            (
                "__AIRBENDER_RUST_TOOLCHAIN_CHANNEL__",
                DEFAULT_GUEST_TOOLCHAIN.to_string(),
            ),
        ]
    }
}

pub(super) fn write_templates(
    destination_root: &Path,
    context: TemplateContext<'_>,
    profile: ProverBackendProfile,
) -> Result<()> {
    let replacements = context.replacements();
    let mut replacement_usage: BTreeMap<&'static str, usize> =
        replacements.iter().map(|(key, _)| (*key, 0usize)).collect();

    for template in template_files(profile) {
        let destination_path = destination_root.join(template.relative_path);
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                CliError::with_source(
                    format!("failed to create directory `{}`", parent.display()),
                    err,
                )
            })?;
        }

        let rendered = render_template(
            template.source,
            template.relative_path,
            &replacements,
            &mut replacement_usage,
        )?;

        fs::write(&destination_path, rendered).map_err(|err| {
            CliError::with_source(
                format!("failed to write `{}`", destination_path.display()),
                err,
            )
        })?;
    }

    let unused_replacements: Vec<_> = replacement_usage
        .into_iter()
        .filter_map(|(key, used)| (used == 0).then_some(key))
        .collect();
    if !unused_replacements.is_empty() {
        return Err(CliError::new(format!(
            "template replacement keys were not used: {}",
            unused_replacements.join(", ")
        ))
        .with_hint("remove stale placeholders or update template replacement mappings"));
    }

    Ok(())
}

fn template_files(profile: ProverBackendProfile) -> [TemplateFile<'static>; 9] {
    [
        TemplateFile {
            relative_path: ".gitignore",
            source: GITIGNORE_TEMPLATE,
        },
        TemplateFile {
            relative_path: "README.md",
            source: ROOT_README_TEMPLATE,
        },
        TemplateFile {
            relative_path: "guest/Cargo.toml",
            source: GUEST_CARGO_TEMPLATE,
        },
        TemplateFile {
            relative_path: "guest/src/main.rs",
            source: GUEST_MAIN_TEMPLATE,
        },
        TemplateFile {
            relative_path: "guest/rust-toolchain.toml",
            source: GUEST_TOOLCHAIN_TEMPLATE,
        },
        TemplateFile {
            relative_path: "guest/.cargo/config.toml",
            source: GUEST_CARGO_CONFIG_TEMPLATE,
        },
        TemplateFile {
            relative_path: "host/Cargo.toml",
            source: HOST_CARGO_TEMPLATE,
        },
        TemplateFile {
            relative_path: "host/src/main.rs",
            source: profile.host_main_template,
        },
        TemplateFile {
            relative_path: "host/rust-toolchain.toml",
            source: HOST_TOOLCHAIN_TEMPLATE,
        },
    ]
}

fn render_template(
    source: &str,
    relative_path: &str,
    replacements: &[(&'static str, String)],
    usage: &mut BTreeMap<&'static str, usize>,
) -> Result<String> {
    let mut rendered = source.to_string();
    for (from, to) in replacements {
        if rendered.contains(from) {
            if let Some(value) = usage.get_mut(from) {
                *value += 1;
            }
            rendered = rendered.replace(from, to);
        }
    }

    if rendered.contains("__AIRBENDER_") {
        return Err(CliError::new(format!(
            "failed to render template `{relative_path}` because unresolved placeholders remain"
        ))
        .with_hint("ensure every __AIRBENDER_*__ token has a replacement mapping"));
    }

    Ok(rendered)
}

fn guest_attributes(enable_std: bool) -> &'static str {
    if enable_std {
        "#![no_main]"
    } else {
        "#![no_std]\n#![no_main]"
    }
}

fn sdk_default_features(allocator: NewAllocatorArg) -> &'static str {
    match allocator {
        NewAllocatorArg::Talc => "",
        NewAllocatorArg::Bump | NewAllocatorArg::Custom => ", default-features = false",
    }
}

fn sdk_features(enable_std: bool, allocator: NewAllocatorArg) -> String {
    let mut sdk_feature_flags = Vec::new();
    if enable_std {
        sdk_feature_flags.push("std");
    }
    match allocator {
        NewAllocatorArg::Talc => {}
        NewAllocatorArg::Bump => sdk_feature_flags.push("allocator-bump"),
        NewAllocatorArg::Custom => sdk_feature_flags.push("allocator-custom"),
    }

    if sdk_feature_flags.is_empty() {
        return String::new();
    }

    let rendered = sdk_feature_flags
        .iter()
        .map(|flag| format!("\"{flag}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(", features = [{rendered}]")
}

fn main_attr_args(allocator: NewAllocatorArg) -> &'static str {
    match allocator {
        NewAllocatorArg::Custom => "(allocator_init = crate::custom_allocator::init)",
        NewAllocatorArg::Talc | NewAllocatorArg::Bump => "",
    }
}

fn custom_allocator_module(allocator: NewAllocatorArg) -> &'static str {
    match allocator {
        NewAllocatorArg::Custom => CUSTOM_ALLOCATOR_MODULE_TEMPLATE,
        NewAllocatorArg::Talc | NewAllocatorArg::Bump => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unresolved_placeholder() {
        let mut usage = BTreeMap::new();
        usage.insert("__AIRBENDER_PROJECT_NAME__", 0);
        let replacements = [("__AIRBENDER_PROJECT_NAME__", "demo".to_string())];

        let err = render_template(
            "__AIRBENDER_PROJECT_NAME__ __AIRBENDER_UNKNOWN__",
            "dummy",
            &replacements,
            &mut usage,
        )
        .expect_err("must fail when unresolved placeholders remain");

        assert!(err.to_string().contains("unresolved placeholders"));
    }
}
