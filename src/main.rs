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

    match fw2tar::main(args) {
        Ok((res, output_path)) => match res {
            BestExtractor::Best(extractor) => {
                println!("xfs: Best extractor: {extractor}, archive at {output_path:?}");
            }
            BestExtractor::Only(extractor) => {
                println!("xfs: Only extractor: {extractor}, archive at {output_path:?}");
            }
            BestExtractor::Identical(extractor) => {
                println!("xfs: Extractors Identical, using {extractor}. Archive at {output_path:?}");
            }
            BestExtractor::None => {
                println!("xfs: No extractor succeeded.");
                exit(2);
            }
        },
        Err(e) => {
            eprintln!("xfs: {e}");
            exit(1);
        }
    }
}
