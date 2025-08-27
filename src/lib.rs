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
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::{env, fs, thread};

use crate::analysis::copy_dir_all;

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
    let output_dir = args.output.clone().unwrap_or_else(|| env::current_dir().unwrap());
    
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
        .clone() // Clone to avoid partial move
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
                    &args,
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
    
    // Print the rootfs path for the best extractor only with relative path
    let relative_rootfs_path = format!("./xfs-extract/{}", best_result.rootfs_path.strip_prefix(extract_dir_path.as_path()).unwrap_or(&best_result.rootfs_path).display());
    
    // Print rootfs path based on mode
    if args.progress {
        println!("xfs: [STAGE 3/4] Selecting best extractor: {}", best_result.extractor);
        println!("xfs: [STAGE 3/4] rootfs found at: {}", relative_rootfs_path);
    } else {
        println!("xfs: rootfs found at: {}", relative_rootfs_path);
    }

    // If copy_rootfs is specified, copy the rootfs directory from the best extractor
    if args.copy_rootfs {
        let target_rootfs_dir = rootfs_dir_path;
        
        // No debug information needed
        
        // Try to copy the rootfs directory, but handle errors gracefully
        match (|| -> Result<(), Fw2tarError> {
            // Remove existing directory if it exists
            if target_rootfs_dir.exists() {
                fs::remove_dir_all(&target_rootfs_dir)?;
            }
            
            // Create parent directories if they don't exist
            if let Some(parent) = target_rootfs_dir.parent() {
                fs::create_dir_all(parent)?;
            }
            
            // In Docker container, we need to handle paths differently
            // Instead of trying to copy directly, we'll use a command to copy
            let source_path = &best_result.rootfs_path;
            
            // Check if we're running in a container (common Docker env var)
            let in_container = env::var("container").is_ok() || Path::new("/.dockerenv").exists();
            
            // Create target directory
            fs::create_dir_all(&target_rootfs_dir)?;
            
            if in_container {
                // Use system cp command which handles Docker volume mounts better
                // Copy contents of the source directory to the target directory
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(format!("cp -a {}/* {}", source_path.display(), target_rootfs_dir.display()))
                    .status()?;
                
                if !status.success() {
                    return Err(Fw2tarError::IoError(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("cp command failed with status: {}", status)
                    )));
                }
            } else {
                // Not in container, use regular copy
                // Check if source exists
                if !source_path.exists() {
                    return Err(Fw2tarError::IoError(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("Source rootfs path does not exist: {}", source_path.display())
                    )));
                }
                
                // Copy the directory contents
                // Read source directory entries
                let entries = fs::read_dir(source_path)?;
                
                // Copy each entry to the target directory
                for entry in entries {
                    let entry = entry?;
                    let path = entry.path();
                    let target = target_rootfs_dir.join(entry.file_name());
                    
                    if path.is_dir() {
                        copy_dir_all(&path, &target)?;
                    } else {
                        fs::copy(&path, &target)?;
                    }
                }
            }
            
            Ok(())
        })() {
            Ok(_) => {
                if args.progress {
                    println!("xfs: [STAGE 4/4] Creating output files");
                    println!("xfs: [STAGE 4/4] rootfs successfully copied to: ./rootfs");
                } else {
                    println!("xfs: rootfs successfully copied to: ./rootfs");
                }
            },
            Err(e) => {
                if args.progress {
                    eprintln!("xfs: [STAGE 4/4] Warning: Failed to copy rootfs directory: {}", e);
                    eprintln!("xfs: [STAGE 4/4] The archive was created successfully, but the rootfs directory couldn't be copied.");
                } else if args.loud {
                    eprintln!("xfs: Warning: Failed to copy rootfs directory: {}", e);
                    eprintln!("xfs: The archive was created successfully, but the rootfs directory couldn't be copied.");
                }
            }
        }
    }

    result
}
