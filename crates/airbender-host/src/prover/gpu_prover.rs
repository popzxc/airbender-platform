use super::{ProveResult, Prover, ProverLevel};
use crate::error::{HostError, Result};
use std::path::{Path, PathBuf};

#[cfg(feature = "gpu-prover")]
use super::{base_path, receipt_from_real_proof, resolve_app_bin_path};
#[cfg(feature = "gpu-prover")]
use crate::proof::{Proof, RealProof};
#[cfg(feature = "gpu-prover")]
use execution_utils::unrolled_gpu::UnrolledProver;
#[cfg(feature = "gpu-prover")]
use gpu_prover::execution::prover::ExecutionProverConfiguration;
#[cfg(feature = "gpu-prover")]
use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;

/// Builder for creating a configured cached GPU prover.
pub struct GpuProverBuilder {
    app_bin_path: PathBuf,
    worker_threads: Option<usize>,
    level: ProverLevel,
}

impl GpuProverBuilder {
    pub fn new(app_bin_path: impl AsRef<Path>) -> Self {
        Self {
            app_bin_path: app_bin_path.as_ref().to_path_buf(),
            worker_threads: None,
            level: ProverLevel::RecursionUnified,
        }
    }

    pub fn with_worker_threads(mut self, worker_threads: usize) -> Self {
        self.worker_threads = Some(worker_threads);
        self
    }

    pub fn with_level(mut self, level: ProverLevel) -> Self {
        self.level = level;
        self
    }

    pub fn build(self) -> Result<GpuProver> {
        GpuProver::new(&self.app_bin_path, self.worker_threads, self.level)
    }
}

/// GPU prover wrapper that owns and reuses a single `UnrolledProver` instance.
pub struct GpuProver {
    #[cfg(feature = "gpu-prover")]
    prover: UnrolledProver,
    level: ProverLevel,
}

#[cfg(feature = "gpu-prover")]
impl GpuProver {
    fn new(app_bin_path: &Path, worker_threads: Option<usize>, level: ProverLevel) -> Result<Self> {
        if matches!(worker_threads, Some(0)) {
            return Err(HostError::Prover(
                "worker thread count must be greater than zero".to_string(),
            ));
        }

        let app_bin_path = resolve_app_bin_path(app_bin_path)?;
        let prover =
            create_unrolled_prover(&app_bin_path, worker_threads, level.as_unrolled_level())?;

        Ok(Self { prover, level })
    }
}

#[cfg(not(feature = "gpu-prover"))]
impl GpuProver {
    fn new(
        _app_bin_path: &Path,
        _worker_threads: Option<usize>,
        _level: ProverLevel,
    ) -> Result<Self> {
        Err(HostError::Prover(
            "GPU prover support is disabled; enable the `gpu-prover` feature on `airbender-host`"
                .to_string(),
        ))
    }
}

#[cfg(feature = "gpu-prover")]
impl Prover for GpuProver {
    fn prove(&self, input_words: &[u32]) -> Result<ProveResult> {
        let oracle = QuasiUARTSource::new_with_reads(input_words.to_vec());
        let (inner_proof, cycles) = self.prover.prove(0, oracle);
        let receipt = receipt_from_real_proof(&inner_proof);
        let proof = Proof::Real(RealProof::new(self.level, inner_proof));

        Ok(ProveResult {
            proof,
            cycles,
            receipt,
        })
    }
}

#[cfg(not(feature = "gpu-prover"))]
impl Prover for GpuProver {
    fn prove(&self, _input_words: &[u32]) -> Result<ProveResult> {
        let _ = self.level;
        Err(HostError::Prover(
            "GPU prover support is disabled; enable the `gpu-prover` feature on `airbender-host`"
                .to_string(),
        ))
    }
}

#[cfg(feature = "gpu-prover")]
fn create_unrolled_prover(
    app_bin_path: &Path,
    worker_threads: Option<usize>,
    level: execution_utils::unrolled_gpu::UnrolledProverLevel,
) -> Result<UnrolledProver> {
    let base_path = base_path(app_bin_path)?;
    let mut configuration = ExecutionProverConfiguration::default();
    if let Some(threads) = worker_threads {
        configuration.max_thread_pool_threads = Some(threads);
        configuration.replay_worker_threads_count = threads;
    }
    Ok(UnrolledProver::new(&base_path, configuration, level))
}
