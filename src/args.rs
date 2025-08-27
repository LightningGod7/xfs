use clap::Parser;
use std::path::PathBuf;

/// Extract firmware images to filesystem archives
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    pub firmware: PathBuf,

    /// Output directory for all artifacts (optional). Default is current directory.
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Disable saving scratch directory (by default scratch directory is saved)
    #[arg(long)]
    pub no_scratch: bool,

    /// Copy out the identified rootfs directory (default: false)
    #[arg(long)]
    pub copy_rootfs: bool,

    /// Comma-separated list of extractors. Supported values are binwalk, binwalkv3, unblob
    #[arg(long)]
    pub extractors: Option<String>,

    /// Enable loud (verbose) output - shows all extraction and processing steps
    #[arg(long)]
    pub loud: bool,

    /// Create a file next to the output file reporting the extractor used
    #[arg(long, alias("report_extractor"))]
    pub report_extractor: bool,

    /// Maximum number of root-like filesystems to extract.
    #[arg(long, default_value_t = 1, alias("primary_limit"))]
    pub primary_limit: usize,

    /// Maximum number of non-root-like filesystems to extract.
    #[arg(long, default_value_t = 0, alias("secondary_limit"))]
    pub secondary_limit: usize,

    /// Overwrite existing output file
    #[arg(long)]
    pub force: bool,

    /// Show help message for the wrapper script
    #[arg(long)]
    pub wrapper_help: bool,

    /// Create a file showing all the devices removed from any of the extractions
    #[arg(long, alias("log_devices"))]
    pub log_devices: bool,

    /// Timeout for extractors, measured in seconds
    #[arg(long, default_value_t = 20)]
    pub timeout: u64,
    
    /// Show detailed progress output with stage information
    #[arg(long)]
    pub progress: bool,
}
