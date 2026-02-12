use crate::cli::NewArgs;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_SDK_GIT_REPOSITORY: &str = "https://github.com/popzxc/airbender-platform";
const DEFAULT_SDK_GIT_BRANCH: &str = "main";

const TEMPLATE_FILES: &[(&str, &str)] = &[
    (
        "README.md",
        include_str!("../../templates/README.md.template"),
    ),
    (
        "guest/Cargo.toml",
        include_str!("../../templates/guest/Cargo.toml.template"),
    ),
    (
        "guest/src/main.rs",
        include_str!("../../templates/guest/src/main.rs.template"),
    ),
    (
        "host/Cargo.toml",
        include_str!("../../templates/host/Cargo.toml.template"),
    ),
    (
        "host/src/main.rs",
        include_str!("../../templates/host/src/main.rs.template"),
    ),
];

pub fn run(args: NewArgs) -> Result<()> {
    let project_name = args.name.or_else(|| {
        args.path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
    });
    let project_name = project_name.context("while attempting to determine project name")?;

    ensure_empty_dir(&args.path)?;
    fs::create_dir_all(&args.path).with_context(|| {
        format!(
            "while attempting to create destination directory {}",
            args.path.display()
        )
    })?;

    let guest_destination_dir = args.path.join("guest");
    fs::create_dir_all(&guest_destination_dir).with_context(|| {
        format!(
            "while attempting to create destination directory {}",
            guest_destination_dir.display()
        )
    })?;
    let host_destination_dir = args.path.join("host");
    fs::create_dir_all(&host_destination_dir).with_context(|| {
        format!(
            "while attempting to create destination directory {}",
            host_destination_dir.display()
        )
    })?;

    let sdk_dependency = resolve_crate_dependency(
        &guest_destination_dir,
        args.sdk_path.as_deref(),
        args.sdk_version.as_deref(),
        "airbender-sdk",
    )?;
    let host_dependency = resolve_crate_dependency(
        &host_destination_dir,
        args.sdk_path.as_deref(),
        args.sdk_version.as_deref(),
        "airbender-host",
    )?;

    let guest_attributes = if args.enable_std {
        "#![no_main]"
    } else {
        "#![no_std]\n#![no_main]"
    };
    let sdk_features = if args.enable_std {
        ", features = [\"std\"]"
    } else {
        ""
    };

    let replacements = [
        ("__AIRBENDER_PROJECT_NAME__", project_name.as_str()),
        ("__AIRBENDER_SDK_DEP__", sdk_dependency.as_str()),
        ("__AIRBENDER_HOST_DEP__", host_dependency.as_str()),
        ("__AIRBENDER_GUEST_ATTRIBUTES__", guest_attributes),
        ("__AIRBENDER_SDK_FEATURES__", sdk_features),
    ];

    write_template(&args.path, &replacements)?;
    tracing::info!("created project at {}", args.path.display());
    Ok(())
}

fn resolve_crate_dependency(
    destination_crate_dir: &Path,
    sdk_path: Option<&Path>,
    sdk_version: Option<&str>,
    crate_name: &str,
) -> Result<String> {
    if let Some(version) = sdk_version {
        if version.is_empty() {
            bail!("--sdk-version cannot be empty");
        }
        return Ok(format!("version = \"{version}\""));
    }

    if let Some(sdk_path) = sdk_path {
        if !sdk_path.exists() {
            bail!("failed to locate SDK path at {}", sdk_path.display());
        }

        let crate_path = resolve_dependency_crate_path(sdk_path, crate_name)?;
        let sdk_relative = relative_path(destination_crate_dir, &crate_path)?;
        return Ok(format!("path = \"{}\"", sdk_relative.to_string_lossy()));
    }

    Ok(format!(
        "git = \"{DEFAULT_SDK_GIT_REPOSITORY}\", branch = \"{DEFAULT_SDK_GIT_BRANCH}\""
    ))
}

fn resolve_dependency_crate_path(sdk_path: &Path, crate_name: &str) -> Result<PathBuf> {
    let mut candidates = Vec::new();

    if sdk_path
        .file_name()
        .map(|name| name == crate_name)
        .unwrap_or(false)
    {
        candidates.push(sdk_path.to_path_buf());
    }
    candidates.push(sdk_path.join(crate_name));
    candidates.push(sdk_path.join("crates").join(crate_name));
    if let Some(parent) = sdk_path.parent() {
        candidates.push(parent.join(crate_name));
    }

    for candidate in candidates {
        if candidate.join("Cargo.toml").exists() {
            return candidate.canonicalize().with_context(|| {
                format!("while attempting to canonicalize {}", candidate.display())
            });
        }
    }

    bail!(
        "failed to locate {crate_name} crate under {}",
        sdk_path.display()
    )
}

fn ensure_empty_dir(path: &Path) -> Result<()> {
    if path.exists()
        && path
            .read_dir()
            .with_context(|| format!("while attempting to list {}", path.display()))?
            .next()
            .is_some()
    {
        bail!("destination directory is not empty: {}", path.display());
    }
    Ok(())
}

fn write_template(destination_root: &Path, replacements: &[(&str, &str)]) -> Result<()> {
    for (relative_path, source) in TEMPLATE_FILES {
        let destination_path = destination_root.join(relative_path);
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("while attempting to create {}", parent.display()))?;
        }

        let mut content = source.to_string();
        for (from, to) in replacements {
            content = content.replace(from, to);
        }
        fs::write(&destination_path, content)
            .with_context(|| format!("while attempting to write {}", destination_path.display()))?;
    }
    Ok(())
}

fn relative_path(from: &Path, to: &Path) -> Result<PathBuf> {
    let from = from
        .canonicalize()
        .with_context(|| format!("while attempting to canonicalize {}", from.display()))?;
    let to = to
        .canonicalize()
        .with_context(|| format!("while attempting to canonicalize {}", to.display()))?;

    let from_components: Vec<_> = from.components().collect();
    let to_components: Vec<_> = to.components().collect();

    let mut common_len = 0usize;
    while common_len < from_components.len()
        && common_len < to_components.len()
        && from_components[common_len] == to_components[common_len]
    {
        common_len += 1;
    }

    let mut result = PathBuf::new();
    for _ in common_len..from_components.len() {
        result.push("..");
    }
    for component in &to_components[common_len..] {
        result.push(component.as_os_str());
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn defaults_to_sdk_git_repository() {
        let dependency = resolve_crate_dependency(Path::new("."), None, None, "airbender-sdk")
            .expect("resolve default SDK dependency");
        assert_eq!(
            dependency,
            "git = \"https://github.com/popzxc/airbender-platform\", branch = \"main\""
        );
    }

    #[test]
    fn prefers_explicit_sdk_version() {
        let dependency =
            resolve_crate_dependency(Path::new("."), None, Some("0.1.0"), "airbender-sdk")
                .expect("resolve version SDK dependency");
        assert_eq!(dependency, "version = \"0.1.0\"");
    }

    #[test]
    fn rejects_empty_sdk_version() {
        let err = resolve_crate_dependency(Path::new("."), None, Some(""), "airbender-sdk")
            .expect_err("empty version should fail");
        assert!(err.to_string().contains("--sdk-version cannot be empty"));
    }

    #[test]
    fn resolves_dependency_from_workspace_root() {
        let root = test_workspace_dir("sdk-workspace-root");
        let destination = root.join("destination").join("guest");
        let sdk_workspace = root.join("sdk-workspace");
        let sdk = sdk_workspace.join("crates").join("airbender-sdk");
        let host = sdk_workspace.join("crates").join("airbender-host");

        fs::create_dir_all(&destination).expect("create destination dir");
        fs::create_dir_all(&sdk).expect("create sdk dir");
        fs::create_dir_all(&host).expect("create host dir");
        fs::write(
            sdk.join("Cargo.toml"),
            "[package]\nname = \"airbender-sdk\"\n",
        )
        .expect("write sdk Cargo.toml");
        fs::write(
            host.join("Cargo.toml"),
            "[package]\nname = \"airbender-host\"\n",
        )
        .expect("write host Cargo.toml");

        let dependency = resolve_crate_dependency(
            &destination,
            Some(sdk_workspace.as_path()),
            None,
            "airbender-sdk",
        )
        .expect("resolve path SDK dependency");
        assert_eq!(
            dependency,
            "path = \"../../sdk-workspace/crates/airbender-sdk\""
        );

        fs::remove_dir_all(&root).expect("remove test directories");
    }

    #[test]
    fn resolves_host_from_sibling_sdk_path() {
        let root = test_workspace_dir("sdk-sibling-host");
        let destination = root.join("destination").join("host");
        let crates_dir = root.join("sdk-workspace").join("crates");
        let sdk = crates_dir.join("airbender-sdk");
        let host = crates_dir.join("airbender-host");

        fs::create_dir_all(&destination).expect("create destination dir");
        fs::create_dir_all(&sdk).expect("create sdk dir");
        fs::create_dir_all(&host).expect("create host dir");
        fs::write(
            sdk.join("Cargo.toml"),
            "[package]\nname = \"airbender-sdk\"\n",
        )
        .expect("write sdk Cargo.toml");
        fs::write(
            host.join("Cargo.toml"),
            "[package]\nname = \"airbender-host\"\n",
        )
        .expect("write host Cargo.toml");

        let dependency =
            resolve_crate_dependency(&destination, Some(sdk.as_path()), None, "airbender-host")
                .expect("resolve host dependency from sibling path");
        assert_eq!(
            dependency,
            "path = \"../../sdk-workspace/crates/airbender-host\""
        );

        fs::remove_dir_all(&root).expect("remove test directories");
    }

    #[test]
    fn new_scaffolds_host_and_guest() {
        let root = test_workspace_dir("scaffold-host-guest");
        let destination = root.join("hello-airbender");

        run(NewArgs {
            path: destination.clone(),
            name: Some("hello-airbender".to_string()),
            enable_std: false,
            sdk_path: None,
            sdk_version: Some("0.1.0".to_string()),
        })
        .expect("create scaffold");

        let guest_cargo =
            fs::read_to_string(destination.join("guest/Cargo.toml")).expect("read guest Cargo");
        let guest_main =
            fs::read_to_string(destination.join("guest/src/main.rs")).expect("read guest main");
        let host_cargo =
            fs::read_to_string(destination.join("host/Cargo.toml")).expect("read host Cargo");
        let host_main =
            fs::read_to_string(destination.join("host/src/main.rs")).expect("read host main");

        assert!(guest_cargo.contains("name = \"hello-airbender-guest\""));
        assert!(guest_cargo.contains("airbender-sdk"));
        assert!(guest_main.contains("#![no_std]"));
        assert!(host_cargo.contains("name = \"hello-airbender-host\""));
        assert!(host_cargo.contains("airbender-host"));
        assert!(host_main.contains("Program::load"));

        fs::remove_dir_all(&root).expect("remove test directories");
    }

    #[test]
    fn new_enable_std_updates_guest_template() {
        let root = test_workspace_dir("scaffold-enable-std");
        let destination = root.join("hello-airbender");

        run(NewArgs {
            path: destination.clone(),
            name: Some("hello-airbender".to_string()),
            enable_std: true,
            sdk_path: None,
            sdk_version: Some("0.1.0".to_string()),
        })
        .expect("create std scaffold");

        let guest_cargo =
            fs::read_to_string(destination.join("guest/Cargo.toml")).expect("read guest Cargo");
        let guest_main =
            fs::read_to_string(destination.join("guest/src/main.rs")).expect("read guest main");

        assert!(guest_cargo.contains("features = [\"std\"]"));
        assert!(!guest_main.contains("#![no_std]"));

        fs::remove_dir_all(&root).expect("remove test directories");
    }

    fn test_workspace_dir(suffix: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tmp")
            .join(format!(
                "cargo-airbender-new-tests-{suffix}-{timestamp}-{}",
                std::process::id()
            ))
    }
}
