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
    copy_rootfs: bool,
    rootfs_dir_path: &Path,
    verbose: bool,
    primary_limit: usize,
    _secondary_limit: usize,
    results: &Mutex<Vec<ExtractionResult>>,
    metadata: &Metadata,
    removed_devices: Option<&Mutex<HashSet<PathBuf>>>,
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

    let log_file = output_dir.join(format!("{extractor_name}.log"));

    let start_time = Instant::now();

    extractor
        .extract(in_file, actual_extract_dir, &log_file, verbose)
        .map_err(ExtractProcessError::ExtractFail)?;

    let elapsed = start_time.elapsed().as_secs_f32();

    if verbose {
        println!("xfs: {extractor_name} took {elapsed:.2} seconds")
    } else {
        log::info!("{extractor_name} took {elapsed:.2} seconds");
    }

    let rootfs_choices = find_linux_filesystems(actual_extract_dir, None, extractor_name);

    if rootfs_choices.is_empty() {
        log::error!("No Linux filesystems found extracting {in_file:?} with {extractor_name}");
        return Err(ExtractProcessError::FailToFind);
    }

    for (i, fs) in rootfs_choices.iter().enumerate() {
        if i >= primary_limit {
            println!(
                "xfs: WARNING: skipping {n} filesystems, if files are missing you may need to set --primary-limit higher",
                n=rootfs_choices.len() - primary_limit
            );
            break;
        }

        // Output the relative path to the identified rootfs directory
        let relative_rootfs_path = if save_scratch {
            let relative_base = extract_dir_base.strip_prefix(output_dir).unwrap_or(extract_dir_base);
            relative_base.join(extractor_name).join(fs.path.strip_prefix(actual_extract_dir).unwrap_or(&fs.path))
        } else {
            // If not saving scratch, just show the temp path info
            fs.path.clone()
        };
        
        println!("xfs: rootfs found at: {}", relative_rootfs_path.display());

        let tar_path = if i == 0 {
            output_dir.join("rootfs.tar.gz")
        } else {
            output_dir.join(format!("rootfs.{i}.tar.gz"))
        };

        // Copy rootfs directory if requested
        if copy_rootfs && i == 0 {
            let target_rootfs_dir = if rootfs_choices.len() > 1 {
                rootfs_dir_path.with_extension(format!("{i}"))
            } else {
                rootfs_dir_path.to_path_buf()
            };
            
            if target_rootfs_dir.exists() {
                std::fs::remove_dir_all(&target_rootfs_dir).unwrap();
            }
            copy_dir_all(&fs.path, &target_rootfs_dir).unwrap();
        }

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

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
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
