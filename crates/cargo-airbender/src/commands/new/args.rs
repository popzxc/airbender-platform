use crate::cli::{NewAllocatorArg, NewArgs, NewProverBackendArg};
use crate::error::{CliError, Result};
use dialoguer::{Confirm, Input, Select};
use std::path::{Component, Path, PathBuf};

pub(super) struct ResolvedNewArgs {
    pub(super) path: PathBuf,
    pub(super) project_name: String,
    pub(super) enable_std: bool,
    pub(super) allocator: NewAllocatorArg,
    pub(super) prover_backend: NewProverBackendArg,
    pub(super) sdk_path: Option<PathBuf>,
    pub(super) sdk_version: Option<String>,
}

pub(super) fn resolve_new_args(args: NewArgs) -> Result<ResolvedNewArgs> {
    let NewArgs {
        path,
        name,
        enable_std,
        allocator,
        prover_backend,
        yes,
        sdk_path,
        sdk_version,
    } = args;

    let path = path.unwrap_or_else(|| PathBuf::from("."));
    ensure_empty_dir(&path)?;

    let inferred_name = infer_project_name(&path)?;
    let default_name = name.or(inferred_name);

    let (project_name, enable_std, allocator, prover_backend) = if yes {
        let project_name = default_name.ok_or_else(|| {
            CliError::new(format!(
                "could not infer project name from destination `{}`",
                path.display()
            ))
            .with_hint("pass an explicit project name with `--name <project-name>`")
        })?;

        (project_name, enable_std, allocator, prover_backend)
    } else {
        ensure_interactive_terminal()?;
        (
            prompt_project_name(default_name.as_deref())?,
            prompt_enable_std(enable_std)?,
            prompt_allocator(allocator)?,
            prompt_prover_backend(prover_backend)?,
        )
    };

    Ok(ResolvedNewArgs {
        path,
        project_name,
        enable_std,
        allocator,
        prover_backend,
        sdk_path,
        sdk_version,
    })
}

fn infer_project_name(path: &Path) -> Result<Option<String>> {
    if let Some(name) = normalize_name(path.file_name()) {
        return Ok(Some(name));
    }

    if is_current_dir_path(path) {
        let current_dir = std::env::current_dir()
            .map_err(|err| CliError::with_source("failed to determine current directory", err))?;
        return Ok(normalize_name(current_dir.file_name()));
    }

    Ok(None)
}

fn is_current_dir_path(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(Component::CurDir))
        && components.all(|component| component == Component::CurDir)
}

fn normalize_name(component: Option<&std::ffi::OsStr>) -> Option<String> {
    component
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty() && value != "." && value != "..")
}

fn ensure_interactive_terminal() -> Result<()> {
    use std::io::IsTerminal;

    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        return Ok(());
    }

    Err(CliError::new("interactive mode requires a terminal")
        .with_hint("pass `--yes` to run non-interactively and provide values via flags"))
}

fn prompt_project_name(default_name: Option<&str>) -> Result<String> {
    let mut prompt = Input::<String>::new().with_prompt("Name of the project");
    if let Some(default_name) = default_name {
        prompt = prompt.with_initial_text(default_name.to_string());
    }

    let project_name = prompt
        .validate_with(|input: &String| {
            if input.trim().is_empty() {
                Err("project name cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(map_prompt_error)?;

    Ok(project_name.trim().to_string())
}

fn prompt_enable_std(default: bool) -> Result<bool> {
    Confirm::new()
        .with_prompt("Enable STD")
        .default(default)
        .interact()
        .map_err(map_prompt_error)
}

fn prompt_allocator(default: NewAllocatorArg) -> Result<NewAllocatorArg> {
    let options = ["talc", "bump", "custom"];
    let default_index = match default {
        NewAllocatorArg::Talc => 0,
        NewAllocatorArg::Bump => 1,
        NewAllocatorArg::Custom => 2,
    };

    let selected = Select::new()
        .with_prompt("Which allocator to use")
        .items(&options)
        .default(default_index)
        .interact()
        .map_err(map_prompt_error)?;

    Ok(match selected {
        0 => NewAllocatorArg::Talc,
        1 => NewAllocatorArg::Bump,
        2 => NewAllocatorArg::Custom,
        _ => {
            return Err(CliError::new(format!(
                "invalid allocator selection index `{selected}`"
            )));
        }
    })
}

fn prompt_prover_backend(default: NewProverBackendArg) -> Result<NewProverBackendArg> {
    let options = [
        "dev (mock proof for development)",
        "gpu (real proving; CUDA required)",
    ];
    let default_index = match default {
        NewProverBackendArg::Dev => 0,
        NewProverBackendArg::Gpu => 1,
    };

    let selected = Select::new()
        .with_prompt("Which prover backend to use")
        .items(&options)
        .default(default_index)
        .interact()
        .map_err(map_prompt_error)?;

    Ok(match selected {
        0 => NewProverBackendArg::Dev,
        1 => NewProverBackendArg::Gpu,
        _ => {
            return Err(CliError::new(format!(
                "invalid prover backend selection index `{selected}`"
            )));
        }
    })
}

fn map_prompt_error(err: dialoguer::Error) -> CliError {
    CliError::with_source("failed to read interactive input", err)
        .with_hint("pass `--yes` to run non-interactively and provide values via flags")
}

fn ensure_empty_dir(path: &Path) -> Result<()> {
    if path.exists()
        && path
            .read_dir()
            .map_err(|err| {
                CliError::with_source(
                    format!("failed to list directory `{}`", path.display()),
                    err,
                )
            })?
            .next()
            .is_some()
    {
        return Err(CliError::new(format!(
            "destination directory `{}` is not empty",
            path.display()
        ))
        .with_hint("choose a new path or remove existing files in that directory"));
    }

    Ok(())
}
