//! Architecture adapter - extensible plugin registry and config for new architectures.
//! Add a new architecture by: 1) implement ArchitecturePlugin, 2) add to arch_config().

/// Per-architecture configuration
#[derive(Debug, Clone)]
pub struct ArchitectureConfig {
    /// Default memory size in bytes
    pub default_memory_size: usize,
    /// Default entry point label (e.g. "_start", "main")
    pub default_entry_label: &'static str,
}

impl Default for ArchitectureConfig {
    fn default() -> Self {
        Self {
            default_memory_size: 64 * 1024,
            default_entry_label: "_start",
        }
    }
}

/// Supported architectures with their configs
pub fn arch_config(arch: &str) -> ArchitectureConfig {
    match arch.to_uppercase().as_str() {
        "LC3" => ArchitectureConfig {
            default_memory_size: 64 * 1024,
            default_entry_label: "_start",
        },
        "MIPS" => ArchitectureConfig {
            default_memory_size: 64 * 1024,
            default_entry_label: "_start",
        },
        "RV32I" | "RISC-V" => ArchitectureConfig {
            default_memory_size: 64 * 1024,
            default_entry_label: "_start",
        },
        _ => ArchitectureConfig::default(),
    }
}

/// List of all supported architecture names (for UI, validation)
pub fn supported_architectures() -> &'static [&'static str] {
    &["RV32I", "LC3", "MIPS"]
}
