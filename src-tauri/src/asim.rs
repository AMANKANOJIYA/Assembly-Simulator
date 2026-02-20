//! ASIM file format - Assembly Simulator project file.
//! Extensible for future architectures: LC-3, MIPS, 8085, 6502, 8086, ARM, x86.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsimFile {
    pub version: u32,
    pub arch: String,
    pub source: String,
    #[serde(default = "default_memory_size")]
    pub memory_size: usize,
    #[serde(default)]
    pub breakpoints: Vec<u32>,
    #[serde(default)]
    pub entry_point: Option<String>,
    #[serde(default = "default_speed")]
    pub speed: u32,
    #[serde(default)]
    pub max_cycle_limit: Option<u64>,
}

fn default_memory_size() -> usize {
    64 * 1024
}

fn default_speed() -> u32 {
    100
}

impl AsimFile {
    pub const VERSION: u32 = 1;
    pub const EXTENSION: &'static str = "asim";

    pub fn new(arch: &str, source: String, memory_size: usize, breakpoints: Vec<u32>, speed: u32, max_cycle_limit: Option<u64>) -> Self {
        Self {
            version: Self::VERSION,
            arch: arch.to_string(),
            source,
            memory_size,
            breakpoints,
            entry_point: Some("_start".to_string()),
            speed,
            max_cycle_limit,
        }
    }
}
