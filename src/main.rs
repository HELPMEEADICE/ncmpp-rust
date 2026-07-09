mod ncm;

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use clap::Parser;

const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

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

    if !Path::new("unlock").exists() {
        fs::create_dir("unlock").expect("Failed to create unlock directory");
    }

    let unlocked_files: HashSet<String> = read_dir_stems("./unlock")
        .unwrap_or_default()
        .into_iter()
        .collect();

    let start = Instant::now();
    let total = AtomicUsize::new(0);
    let unlocked_files = Arc::new(unlocked_files);
    let log_mtx = Arc::new(Mutex::new(()));

    let ncm_files: Vec<_> = match read_dir_ncm("./") {
        Ok(files) => files,
        Err(e) => {
            eprintln!("{RED}Error: Failed to read current directory: {e}{RESET}");
            return;
        }
    };

    let files_iter = Arc::new(Mutex::new(ncm_files.into_iter()));

    std::thread::scope(|s| {
        for _ in 0..threads {
            let files_iter = Arc::clone(&files_iter);
            let unlocked_files = Arc::clone(&unlocked_files);
            let log_mtx = Arc::clone(&log_mtx);
            let total = &total;
            s.spawn(move || {
                loop {
                    let path = {
                        let mut iter = files_iter.lock().unwrap();
                        iter.next()
                    };
                    let path = match path {
                        Some(p) => p,
                        None => break,
                    };

                    let stem = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let filename = path
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();

                    if unlocked_files.contains(&stem) {
                        let _lock = log_mtx.lock().unwrap();
                        println!("{YELLOW}Skipped:\t{filename}{RESET}");
                        continue;
                    }

                    match ncm::ncm_dump(&path, "unlock") {
                        Ok(()) => {
                            let _lock = log_mtx.lock().unwrap();
                            println!("{CYAN}Unlocked:\t{filename}{RESET}");
                            total.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => {
                            let _lock = log_mtx.lock().unwrap();
                            eprintln!("{RED}Failed:\t{filename} ({e}){RESET}");
                        }
                    }
                }
            });
        }
    });

    let elapsed = start.elapsed();
    println!("\n{GREEN}Finished.{RESET}");
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
