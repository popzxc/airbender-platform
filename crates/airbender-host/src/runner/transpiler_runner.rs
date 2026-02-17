use super::{resolve_cycles, ExecutionResult, Runner};
use crate::error::{HostError, Result};
use crate::receipt::Receipt;
use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
use riscv_transpiler::common_constants::{
    rom::ROM_SECOND_WORD_BITS, INITIAL_TIMESTAMP, TIMESTAMP_STEP,
};
use riscv_transpiler::ir::{preprocess_bytecode, FullUnsignedMachineDecoderConfig};
#[cfg(target_arch = "x86_64")]
use riscv_transpiler::jit::JittedCode;
use riscv_transpiler::jit::RAM_SIZE;
use riscv_transpiler::vm::{DelegationsCounters, RamWithRomRegion, SimpleTape, State, VM};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Builder for creating a configured transpiler runner.
pub struct TranspilerRunnerBuilder {
    app_bin_path: PathBuf,
    cycles: Option<usize>,
    text_path: Option<PathBuf>,
    use_jit: bool,
}

impl TranspilerRunnerBuilder {
    pub fn new(app_bin_path: impl AsRef<Path>) -> Self {
        Self {
            app_bin_path: app_bin_path.as_ref().to_path_buf(),
            cycles: None,
            text_path: None,
            use_jit: false,
        }
    }

    pub fn with_cycles(mut self, cycles: usize) -> Self {
        self.cycles = Some(cycles);
        self
    }

    pub fn with_text_path(mut self, text_path: impl AsRef<Path>) -> Self {
        self.text_path = Some(text_path.as_ref().to_path_buf());
        self
    }

    pub fn with_jit(mut self) -> Self {
        self.use_jit = true;
        self
    }

    pub fn build(self) -> Result<TranspilerRunner> {
        if self.use_jit && cfg!(not(target_arch = "x86_64")) {
            return Err(HostError::Transpiler(
                "JIT execution is only available on x86_64 targets".to_string(),
            ));
        }

        let app_bin_path = resolve_app_bin_path(&self.app_bin_path)?;
        let app_text_path = self
            .text_path
            .as_deref()
            .map(resolve_text_path)
            .unwrap_or_else(|| resolve_text_path(&derive_text_path(&app_bin_path)))?;
        let cycles = resolve_cycles(self.cycles)?;

        Ok(TranspilerRunner {
            app_bin_path,
            app_text_path,
            cycles,
            use_jit: self.use_jit,
        })
    }
}

/// Transpiler based execution runner.
pub struct TranspilerRunner {
    app_bin_path: PathBuf,
    app_text_path: PathBuf,
    cycles: usize,
    use_jit: bool,
}

impl Runner for TranspilerRunner {
    fn run(&self, input_words: &[u32]) -> Result<ExecutionResult> {
        if self.use_jit {
            return self.run_with_jit(input_words);
        }

        self.run_without_jit(input_words)
    }
}

impl TranspilerRunner {
    #[cfg(target_arch = "x86_64")]
    fn run_with_jit(&self, input_words: &[u32]) -> Result<ExecutionResult> {
        let bin_words = read_u32_words(&self.app_bin_path)?;
        let text_words = read_u32_words(&self.app_text_path)?;
        let mut non_determinism_source = QuasiUARTSource::new_with_reads(input_words.to_vec());

        let cycles_bound = match u32::try_from(self.cycles) {
            Ok(value) => Some(value),
            Err(_) => {
                tracing::warn!(
                    "cycles limit {} exceeds u32::MAX; running transpiler without a cycle bound",
                    self.cycles
                );
                None
            }
        };

        let (state, _memory) = JittedCode::run_alternative_simulator(
            &text_words,
            &mut non_determinism_source,
            &bin_words,
            cycles_bound,
        );
        let cycles_executed = ((state.timestamp - INITIAL_TIMESTAMP) / TIMESTAMP_STEP) as usize;

        Ok(ExecutionResult {
            receipt: Receipt::from_registers(state.registers),
            cycles_executed,
            reached_end: true,
        })
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn run_with_jit(&self, _input_words: &[u32]) -> Result<ExecutionResult> {
        Err(HostError::Transpiler(
            "JIT execution is only available on x86_64 targets".to_string(),
        ))
    }

    fn run_without_jit(&self, input_words: &[u32]) -> Result<ExecutionResult> {
        let bin_words = read_u32_words(&self.app_bin_path)?;
        let text_words = read_u32_words(&self.app_text_path)?;
        let instructions = preprocess_bytecode::<FullUnsignedMachineDecoderConfig>(&text_words);
        let instruction_tape = SimpleTape::new(&instructions);
        let mut ram =
            RamWithRomRegion::<{ ROM_SECOND_WORD_BITS }>::from_rom_content(&bin_words, RAM_SIZE);
        let mut state = State::initial_with_counters(DelegationsCounters::default());
        let mut non_determinism_source = QuasiUARTSource::new_with_reads(input_words.to_vec());

        let reached_end = VM::<DelegationsCounters>::run_basic_unrolled::<_, _, _>(
            &mut state,
            &mut ram,
            &mut (),
            &instruction_tape,
            self.cycles,
            &mut non_determinism_source,
        );
        let cycles_executed = ((state.timestamp - INITIAL_TIMESTAMP) / TIMESTAMP_STEP) as usize;
        let registers = state.registers.map(|register| register.value);

        Ok(ExecutionResult {
            receipt: Receipt::from_registers(registers),
            cycles_executed,
            reached_end,
        })
    }
}

fn resolve_app_bin_path(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Err(HostError::Transpiler(format!(
            "binary not found: {}",
            path.display()
        )));
    }

    path.canonicalize().map_err(|err| {
        HostError::Transpiler(format!(
            "failed to canonicalize binary path {}: {err}",
            path.display()
        ))
    })
}

fn resolve_text_path(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Err(HostError::Transpiler(format!(
            "text file not found: {}",
            path.display()
        )));
    }

    path.canonicalize().map_err(|err| {
        HostError::Transpiler(format!(
            "failed to canonicalize text path {}: {err}",
            path.display()
        ))
    })
}

fn derive_text_path(bin_path: &Path) -> PathBuf {
    let mut text_path = bin_path.to_path_buf();
    text_path.set_extension("text");
    text_path
}

fn read_u32_words(path: &Path) -> Result<Vec<u32>> {
    let mut file = std::fs::File::open(path).map_err(|err| {
        HostError::Transpiler(format!("failed to open {}: {err}", path.display()))
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|err| {
        HostError::Transpiler(format!("failed to read {}: {err}", path.display()))
    })?;

    if bytes.len() % 4 != 0 {
        return Err(HostError::Transpiler(format!(
            "file length is not a multiple of 4: {}",
            path.display()
        )));
    }

    let mut words = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.as_chunks::<4>().0 {
        words.push(u32::from_le_bytes(*chunk));
    }
    Ok(words)
}
