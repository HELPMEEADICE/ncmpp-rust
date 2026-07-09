mod ncm;

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use clap::Parser;
use colored::*;
use rayon::prelude::*;

#[derive(Parser)]
#[command(name = "ncmpp", about = "A fast multi-threaded NCM decrypter")]
struct Cli {
    #[arg(short = 't', long = "threads", help = "Max count of unlock threads")]
    threads: Option<usize>,

    #[arg(
        short = 's',
        long = "showtime",
        help = "Shows how long it took to unlock everything"
    )]
    showtime: bool,
}

fn main() {
    let cli = Cli::parse();

    let threads = cli
        .threads
        .unwrap_or_else(|| available_parallelism().unwrap_or(2));

    println!("Start with {} threads.\n", threads);

    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .expect("Failed to build thread pool");

    if !Path::new("unlock").exists() {
        fs::create_dir("unlock").expect("Failed to create unlock directory");
    }

    let unlocked_files: HashSet<String> = read_dir_stems("./unlock")
        .unwrap_or_default()
        .into_iter()
        .collect();

    let start = Instant::now();
    let total = AtomicUsize::new(0);

    let ncm_files: Vec<_> = match read_dir_ncm("./") {
        Ok(files) => files,
        Err(e) => {
            eprintln!("{} Failed to read current directory: {}", "Error:".red(), e);
            return;
        }
    };

    ncm_files.par_iter().for_each(|path| {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        if unlocked_files.contains(&stem) {
            println!("{}", format!("Skipped:\t{}", filename).yellow());
        } else {
            match ncm::ncm_dump(path, "unlock") {
                Ok(()) => {
                    println!("{}", format!("Unlocked:\t{}", filename).cyan());
                    total.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    eprintln!("{}", format!("Failed:\t{} ({})", filename, e).red());
                }
            }
        }
    });

    let elapsed = start.elapsed();
    println!("\n{}", "Finished.".green());
    println!(
        "Unlocked {} pieces of music.",
        total.load(Ordering::Relaxed)
    );

    if cli.showtime {
        println!("Time elapsed: {:.3}s", elapsed.as_secs_f64());
    }
}

fn available_parallelism() -> Option<usize> {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .ok()
        .filter(|&n| n > 0)
}

fn read_dir_ncm(dir: &str) -> io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |e| e == "ncm") {
            files.push(path);
        }
    }
    Ok(files)
}

fn read_dir_stems(dir: &str) -> io::Result<Vec<String>> {
    let mut stems = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(stem) = path.file_stem() {
                stems.push(stem.to_string_lossy().into_owned());
            }
        }
    }
    Ok(stems)
}
