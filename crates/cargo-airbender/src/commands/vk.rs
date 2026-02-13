use crate::cli::{GenerateVkArgs, ProverLevelArg, VerifyProofArgs};
use crate::error::{CliError, Result};
use crate::ui;
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;

pub fn generate(args: GenerateVkArgs) -> Result<()> {
    let vk = match args.level {
        ProverLevelArg::RecursionUnified => {
            let vk = airbender_host::compute_unified_vk(&args.app_bin).map_err(|err| {
                CliError::with_source(
                    format!(
                        "failed to compute unified verification keys for `{}`",
                        args.app_bin.display()
                    ),
                    err,
                )
            })?;
            airbender_host::VerificationKey::RealUnified(
                airbender_host::RealUnifiedVerificationKey { vk },
            )
        }
        ProverLevelArg::Base | ProverLevelArg::RecursionUnrolled => {
            let level = as_host_level(args.level);
            let vk = airbender_host::compute_unrolled_vk(&args.app_bin, level).map_err(|err| {
                CliError::with_source(
                    format!(
                        "failed to compute unrolled verification keys for `{}`",
                        args.app_bin.display()
                    ),
                    err,
                )
            })?;
            airbender_host::VerificationKey::RealUnrolled(
                airbender_host::RealUnrolledVerificationKey { level, vk },
            )
        }
    };

    write_bincode(&args.output, &vk)?;

    ui::success("verification keys generated");
    ui::field("level", level_name(args.level));
    ui::field("output", args.output.display());

    Ok(())
}

pub fn verify(args: VerifyProofArgs) -> Result<()> {
    let proof: airbender_host::Proof = read_bincode(&args.proof).map_err(|err| {
        CliError::with_source(
            format!("failed to decode proof from `{}`", args.proof.display()),
            err,
        )
    })?;

    let vk: airbender_host::VerificationKey = read_bincode(&args.vk).map_err(|err| {
        CliError::with_source(
            format!(
                "failed to decode verification key file `{}`",
                args.vk.display()
            ),
            err,
        )
    })?;

    let level = match &proof {
        airbender_host::Proof::Dev(_) => {
            return Err(CliError::new(
                "detected a dev proof; `cargo airbender verify-proof` supports only real proofs",
            )
            .with_hint(
                "verify dev proofs through `airbender-host` with `Program::dev_verifier()`",
            ));
        }
        airbender_host::Proof::Real(proof) => {
            airbender_host::verify_real_proof_with_vk(proof, &vk)
                .map_err(|err| CliError::with_source("proof verification failed", err))?;
            proof.level()
        }
    };

    ui::success("proof verified");
    ui::field("level", host_level_name(level));

    Ok(())
}

fn as_host_level(level: ProverLevelArg) -> airbender_host::ProverLevel {
    match level {
        ProverLevelArg::Base => airbender_host::ProverLevel::Base,
        ProverLevelArg::RecursionUnrolled => airbender_host::ProverLevel::RecursionUnrolled,
        ProverLevelArg::RecursionUnified => airbender_host::ProverLevel::RecursionUnified,
    }
}

fn level_name(level: ProverLevelArg) -> &'static str {
    match level {
        ProverLevelArg::Base => "base",
        ProverLevelArg::RecursionUnrolled => "recursion-unrolled",
        ProverLevelArg::RecursionUnified => "recursion-unified",
    }
}

fn host_level_name(level: airbender_host::ProverLevel) -> &'static str {
    match level {
        airbender_host::ProverLevel::Base => "base",
        airbender_host::ProverLevel::RecursionUnrolled => "recursion-unrolled",
        airbender_host::ProverLevel::RecursionUnified => "recursion-unified",
    }
}

fn read_bincode<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = std::fs::read(path).map_err(|err| {
        CliError::with_source(format!("failed to read `{}`", path.display()), err)
    })?;
    let (decoded, read_len): (T, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
            .map_err(|err| CliError::with_source("failed to decode bincode payload", err))?;

    if read_len != bytes.len() {
        tracing::warn!(
            "bincode decoded {} bytes but file is {} bytes",
            read_len,
            bytes.len()
        );
    }
    Ok(decoded)
}

fn write_bincode<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let encoded = bincode::serde::encode_to_vec(value, bincode::config::standard())
        .map_err(|err| CliError::with_source("failed to encode bincode payload", err))?;
    std::fs::write(path, encoded).map_err(|err| {
        CliError::with_source(format!("failed to write `{}`", path.display()), err)
    })?;

    Ok(())
}
