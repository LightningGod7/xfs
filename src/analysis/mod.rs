use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;
use std::{env, fs, io};

use sha1::{Digest, Sha1};
use tempfile::TempDir;
use thiserror::Error;

pub mod directory_executables;
pub mod find_linux_filesystems;

use crate::archive::tar_fs;
use crate::extractors::{ExtractError, Extractor};
use crate::metadata::Metadata;
use find_linux_filesystems::find_linux_filesystems;

#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub extractor: &'static str,
    pub index: usize,
    pub size: u64,
    pub num_files: usize,
    pub primary: bool,
    pub archive_hash: String,
    pub file_node_count: usize,
    pub path: PathBuf,
    pub rootfs_path: PathBuf, // Path to the rootfs directory
}

#[derive(Error, Debug)]
pub enum ExtractProcessError {
    #[error("Failed to create temporary directory ({0:?})")]
    TempDirFail(io::Error),

    #[error("Failed to extract from file with extractor ({0})")]
    ExtractFail(ExtractError),

    #[error("Failed to find any filesystems in the extracted contents")]
    FailToFind,
}

pub fn extract_and_process(
    extractor: &dyn Extractor,
    in_file: &Path,
    output_dir: &Path,
    extract_dir_base: &Path,
    save_scratch: bool,
    _copy_rootfs: bool,
    _rootfs_dir_path: &Path,
    verbose: bool,
    primary_limit: usize,
    _secondary_limit: usize,
    results: &Mutex<Vec<ExtractionResult>>,
    metadata: &Metadata,
    removed_devices: Option<&Mutex<HashSet<PathBuf>>>,
    args: &crate::args::Args,
) -> Result<(), ExtractProcessError> {
    let extractor_name = extractor.name();

    // Create extract directory based on extractor name
    let extract_dir = extract_dir_base.join(extractor_name);
    
    // Create the extract directory if it doesn't exist
    if !extract_dir.exists() {
        std::fs::create_dir_all(&extract_dir).map_err(ExtractProcessError::TempDirFail)?;
    }

    // For scratch directory, we'll handle cleanup ourselves if save_scratch is false
    let temp_dir = if save_scratch {
        None
    } else {
        let temp_dir_prefix = format!("xfs_{extractor_name}");
        Some(TempDir::with_prefix_in(temp_dir_prefix, env::temp_dir())
            .map_err(ExtractProcessError::TempDirFail)?)
    };

    let actual_extract_dir = if save_scratch {
        &extract_dir
    } else {
        temp_dir.as_ref().unwrap().path()
    };

    // Only create log file if --logs flag is specified
    let log_file = if args.logs {
        output_dir.join(format!("{extractor_name}.log"))
    } else {
        // Use a temporary path that will be discarded
        env::temp_dir().join(format!("{extractor_name}_{}.log", std::process::id()))
    };

    let start_time = Instant::now();

    // Print extraction status if progress flag is enabled
    if args.progress {
        println!("xfs: [STAGE 1/4] {} - extraction: starting...", extractor_name);
    } else if verbose {
        // Only print extraction status in verbose mode for non-progress output
        print!("xfs: {} - extraction: ", extractor_name);
    }
    
    let extraction_result = extractor
        .extract(in_file, actual_extract_dir, &log_file, verbose);
    
    if extraction_result.is_ok() {
        if args.progress {
            println!("xfs: [STAGE 1/4] {} - extraction: completed ✓", extractor_name);
        } else if verbose {
            println!("✓");
        }
    } else {
        if args.progress || verbose {
            println!("✗");
        }
        return Err(ExtractProcessError::ExtractFail(extraction_result.err().unwrap()));
    }

    let elapsed = start_time.elapsed().as_secs_f32();
    log::info!("{extractor_name} took {elapsed:.2} seconds");

    // Print rootfs finding status if progress flag is enabled
    if args.progress {
        println!("xfs: [STAGE 2/4] {} - identify rootfs: searching...", extractor_name);
    } else if verbose {
        // Only print rootfs finding status in verbose mode for non-progress output
        print!("xfs: {} - identify rootfs: ", extractor_name);
    }
    
    let rootfs_choices = find_linux_filesystems(actual_extract_dir, None, extractor_name);

    if rootfs_choices.is_empty() {
        // if args.progress || verbose {
        //     println!("✗");
        // }
        println!("\txfs: [STAGE 2/4] {} - identify rootfs: No Linux rootfs found ✗", extractor_name);
        // log::error!("No Linux filesystems found extracting {in_file:?} with {extractor_name}");
        return Err(ExtractProcessError::FailToFind);
    } else {
        if args.progress {
            println!("\txfs: [STAGE 2/4] {} - identify rootfs: found ✓", extractor_name);
        } else if verbose {
            println!("✓");
        }
    }

    for (i, fs) in rootfs_choices.iter().enumerate() {
        if i >= primary_limit {
            if args.progress {
                println!(
                    "xfs: [STAGE 2/4] WARNING: skipping {n} filesystems, if files are missing you may need to set --primary-limit higher",
                    n=rootfs_choices.len() - primary_limit
                );
            } else if verbose {
                println!(
                    "xfs: WARNING: skipping {n} filesystems, if files are missing you may need to set --primary-limit higher",
                    n=rootfs_choices.len() - primary_limit
                );
            }
            break;
        }

        // Output the relative path to the identified rootfs directory
        let _relative_rootfs_path = if save_scratch {
            let relative_base = extract_dir_base.strip_prefix(output_dir).unwrap_or(extract_dir_base);
            relative_base.join(extractor_name).join(fs.path.strip_prefix(actual_extract_dir).unwrap_or(&fs.path))
        } else {
            // If not saving scratch, just show the temp path info
            fs.path.clone()
        };
        
        // Only print rootfs path for the best extractor later

        let tar_path = if i == 0 {
            output_dir.join("rootfs.tar.gz")
        } else {
            output_dir.join(format!("rootfs.{i}.tar.gz"))
        };

        // We'll copy the rootfs directory later if needed, after determining the best extractor

        // XXX: improve error handling here
        let file_node_count = tar_fs(&fs.path, &tar_path, metadata, removed_devices).unwrap();
        let archive_hash = sha1_file(&tar_path).unwrap();

        results.lock().unwrap().push(ExtractionResult {
            extractor: extractor_name,
            index: i,
            size: fs.size,
            num_files: fs.num_files,
            primary: true,
            archive_hash,
            file_node_count,
            path: tar_path,
            rootfs_path: fs.path.clone(),
        });
    }

    drop(temp_dir);

    Ok(())
}

pub fn sha1_file(file: &Path) -> io::Result<String> {
    let bytes = std::fs::read(file)?;

    let mut hasher = Sha1::new();
    hasher.update(&bytes[..]);
    let result = hasher.finalize();

    Ok(format!("{result:x}"))
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
