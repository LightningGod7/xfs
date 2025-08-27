use std::process::exit;

use clap::Parser;

use fw2tar::args::Args;
use fw2tar::BestExtractor;

fn main() {
    let args = Args::parse();

    if args.loud && std::env::var("XFS_LOG").is_err() {
        std::env::set_var("XFS_LOG", "debug");
    }

    pretty_env_logger::init_custom_env("XFS_LOG");

    match fw2tar::main(args.clone()) {
        Ok((res, _output_path)) => match res {
            BestExtractor::Best(extractor) => {
                // Always print the best extractor information
                println!("xfs: best extractor: {extractor}");
                
                
                // Print archive information
                if args.progress {
                    println!("xfs: [STAGE 4/4] rootfs archive created: ./rootfs.tar.gz");
                    println!("xfs: Process complete");
                } else {
                    println!("xfs: rootfs archive: ./rootfs.tar.gz");
                }
            }
            BestExtractor::Only(extractor) => {
                // Always print the best extractor information
                println!("xfs: best extractor: {extractor}");
                
                
                // Print archive information
                if args.progress {
                    println!("xfs: [STAGE 4/4] rootfs archive created: ./rootfs.tar.gz");
                    println!("xfs: Process complete");
                } else {
                    println!("xfs: rootfs archive: ./rootfs.tar.gz");
                }
            }
            BestExtractor::Identical(extractor) => {
                // Always print the best extractor information
                println!("xfs: best extractor: {extractor}");
                
                
                // Print archive information
                if args.progress {
                    println!("xfs: [STAGE 4/4] rootfs archive created: ./rootfs.tar.gz");
                    println!("xfs: Process complete");
                } else {
                    println!("xfs: rootfs archive: ./rootfs.tar.gz");
                }
            }
            BestExtractor::None => {
                println!("xfs: no extractor succeeded");
                exit(2);
            }
        },
        Err(e) => {
            eprintln!("xfs: {e}");
            exit(1);
        }
    }
}
