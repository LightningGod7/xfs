pub mod analysis;
pub mod archive;
pub mod args;
mod error;
pub mod extractors;
pub mod metadata;

use analysis::{extract_and_process, ExtractionResult};
pub use error::Fw2tarError;
use metadata::Metadata;

use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::{env, fs, thread};

pub enum BestExtractor {
    Best(&'static str),
    Only(&'static str),
    Identical(&'static str),
    None,
}

pub fn main(args: args::Args) -> Result<(BestExtractor, PathBuf), Fw2tarError> {
    if !args.firmware.is_file() {
        if args.firmware.exists() {
            return Err(Fw2tarError::FirmwareNotAFile(args.firmware));
        } else {
            return Err(Fw2tarError::FirmwareDoesNotExist(args.firmware));
        }
    }

    // Determine output directory - default to current directory
    let output_dir = args.output.unwrap_or_else(|| env::current_dir().unwrap());
    
    // Ensure output directory exists
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir)?;
    }

    // Extract base filename from firmware (for future use)
    let _firmware_base = if let Some(stem) = args.firmware.file_stem() {
        stem.to_string_lossy().to_string()
    } else {
        args.firmware.file_name().unwrap().to_string_lossy().to_string()
    };

    // Set up output paths
    let selected_output_path = output_dir.join("rootfs.tar.gz");
    let extract_dir_path = output_dir.join("xfs-extract");
    let rootfs_dir_path = output_dir.join("rootfs");

    // Check if either rootfs.tar.gz or xfs-extract directory already exist
    if (selected_output_path.exists() || extract_dir_path.exists()) && !args.force {
        // Return error with the path that exists (prioritize rootfs.tar.gz if both exist)
        if selected_output_path.exists() {
            return Err(Fw2tarError::OutputExists(selected_output_path));
        } else {
            return Err(Fw2tarError::OutputExists(extract_dir_path));
        }
    }
    
    // If --force is specified, remove existing files/directories
    if args.force {
        // Remove rootfs.tar.gz if it exists
        if selected_output_path.exists() {
            fs::remove_file(&selected_output_path)?;
        }
        
        // Remove xfs-extract directory if it exists
        if extract_dir_path.exists() {
            fs::remove_dir_all(&extract_dir_path)?;
        }
    }

    let metadata = Metadata {
        input_hash: analysis::sha1_file(&args.firmware).unwrap_or_default(),
        file: args.firmware.display().to_string(),
        fw2tar_command: env::args().collect(),
    };

    extractors::set_timeout(args.timeout);

    let extractors: Vec<_> = args
        .extractors
        .map(|extractors| extractors.split(",").map(String::from).collect())
        .unwrap_or_else(|| {
            extractors::all_extractor_names()
                .map(String::from)
                .collect()
        });

    let results: Mutex<Vec<ExtractionResult>> = Mutex::new(Vec::new());

    let removed_devices: Option<Mutex<HashSet<PathBuf>>> =
        args.log_devices.then(|| Mutex::new(HashSet::new()));

    thread::scope(|threads| -> Result<(), Fw2tarError> {
        for extractor_name in extractors {
            let extractor = extractors::get_extractor(&extractor_name)
                .ok_or_else(|| Fw2tarError::InvalidExtractor(extractor_name.clone()))?;

            threads.spawn(|| {
                if let Err(e) = extract_and_process(
                    extractor,
                    &args.firmware,
                    &output_dir,
                    &extract_dir_path,
                    !args.no_scratch,
                    args.copy_rootfs,
                    &rootfs_dir_path,
                    args.loud,
                    args.primary_limit,
                    args.secondary_limit,
                    &results,
                    &metadata,
                    removed_devices.as_ref(),
                ) {
                    log::info!("{} error: {e}", extractor.name());
                }
            });
        }

        Ok(())
    })?;

    if let Some(removed_devices) = removed_devices {
        let mut removed_devices = removed_devices
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        removed_devices.sort();

        if removed_devices.is_empty() {
            log::warn!("No device files were found during extraction, skipping writing log");
        } else {
            let devices_log_path = output_dir.join("devices.log");
            fs::write(
                devices_log_path,
                removed_devices.join("\n"),
            )
            .unwrap();
        }
    }

    let results = results.lock().unwrap();
    let mut best_results: Vec<_> = results.iter().filter(|&res| res.index == 0).collect();

    let result = if best_results.is_empty() {
        return Ok((BestExtractor::None, selected_output_path));
    } else if best_results.len() == 1 {
        Ok((BestExtractor::Only(best_results[0].extractor), selected_output_path.clone()))
    } else {
        best_results.sort_by_key(|res| Reverse((res.file_node_count, res.extractor == "unblob")));

        Ok((BestExtractor::Best(best_results[0].extractor), selected_output_path.clone()))
    };

    let best_result = best_results[0];

    fs::rename(&best_result.path, &selected_output_path).unwrap();

    result
}
