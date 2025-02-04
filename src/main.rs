mod config;
mod processor;

use clap::{Parser, Subcommand};
use colored::*;
use config::Config;
use notify_debouncer_mini::{new_debouncer, notify::*};
use processor::{process_single_file, FileProcessor};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Watch for file changes
    #[arg(short, long)]
    watch: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate TypeScript enums from C# enum files
    Enums {
        /// Input directory containing C# enum files
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Output directory for TypeScript files
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate Zod schemas from C# DTOs
    Schemas {
        /// Input directory containing C# DTO files
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Output directory for TypeScript files
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Generate localized schemas
        #[arg(short, long)]
        localized: bool,
    },
}

async fn watch_directory(
    path: PathBuf,
    event_tx: mpsc::Sender<PathBuf>,
    config: Config,
) -> notify::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        move |events: notify_debouncer_mini::DebounceEventResult| {
            if let Ok(events) = events {
                for event in events {
                    if let Ok(()) = tx.send(event.path) {
                        // Successfully sent the event
                    }
                }
            }
        },
    )?;

    debouncer.watcher().watch(&path, RecursiveMode::Recursive)?;

    println!(
        "{}",
        format!("👀 Watching for changes in {}...", path.display()).cyan()
    );

    loop {
        if let Ok(modified_path) = rx.recv() {
            if config.is_valid_extension(&modified_path) && !config.should_ignore(&modified_path) {
                let _ = event_tx.send(modified_path);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let config = Config::load().unwrap_or_default();

    match cli.command {
        Commands::Enums { input, output } => {
            let config = config.clone();
            let input_dir = input
                .or_else(|| config.input_dir.clone())
                .expect("Input directory is required");
            let output_dir = output
                .or_else(|| config.output_dir.clone())
                .expect("Output directory is required");

            if cli.watch {
                let (tx, rx) = mpsc::channel();
                let input_clone = input_dir.clone();
                let config_clone = config.clone();

                tokio::spawn(async move {
                    if let Err(e) = watch_directory(input_clone, tx, config_clone).await {
                        eprintln!("{}: {}", "Watch error".red(), e);
                    }
                });

                let mut processor = FileProcessor::new();
                println!("{}", "Processing C# enums...".green());
                if let Err(e) =
                    process_single_file(&mut processor, &input_dir, &output_dir, &config)
                {
                    eprintln!("{}: {}", "Error".red(), e);
                }
                processor.stats.print_summary();

                loop {
                    if let Ok(modified_path) = rx.recv() {
                        println!(
                            "{}",
                            format!("🔄 File changed: {}", modified_path.display()).yellow()
                        );
                        let mut processor = FileProcessor::new(); // Reset stats for each change
                        if let Err(e) = process_single_file(
                            &mut processor,
                            &modified_path,
                            &output_dir,
                            &config,
                        ) {
                            eprintln!("{}: {}", "Error".red(), e);
                        } else {
                            println!(
                                "{}",
                                "✨ TypeScript enums regenerated successfully!".green()
                            );
                            processor.stats.print_summary();
                        }
                    }
                }
            } else {
                let mut processor = FileProcessor::new();
                println!("{}", "Processing C# enums...".green());
                if let Err(e) =
                    process_single_file(&mut processor, &input_dir, &output_dir, &config)
                {
                    eprintln!("{}: {}", "Error".red(), e);
                    std::process::exit(1);
                }
                println!("{}", "✨ TypeScript enums generated successfully!".green());
                processor.stats.print_summary();
            }
        }
        Commands::Schemas {
            input,
            output,
            localized,
        } => {
            let mut config = config.clone();
            config.localized = localized || config.localized;

            let input_dir = input
                .or_else(|| config.input_dir.clone())
                .expect("Input directory is required");
            let output_dir = output
                .or_else(|| config.output_dir.clone())
                .expect("Output directory is required");

            if cli.watch {
                let (tx, rx) = mpsc::channel();
                let input_clone = input_dir.clone();
                let config_clone = config.clone();

                tokio::spawn(async move {
                    if let Err(e) = watch_directory(input_clone, tx, config_clone).await {
                        eprintln!("{}: {}", "Watch error".red(), e);
                    }
                });

                let mut processor = FileProcessor::new();
                println!("{}", "Processing C# DTOs...".green());
                if let Err(e) =
                    process_single_file(&mut processor, &input_dir, &output_dir, &config)
                {
                    eprintln!("{}: {}", "Error".red(), e);
                }
                processor.stats.print_summary();

                loop {
                    if let Ok(modified_path) = rx.recv() {
                        println!(
                            "{}",
                            format!("🔄 File changed: {}", modified_path.display()).yellow()
                        );
                        let mut processor = FileProcessor::new(); // Reset stats for each change
                        if let Err(e) = process_single_file(
                            &mut processor,
                            &modified_path,
                            &output_dir,
                            &config,
                        ) {
                            eprintln!("{}: {}", "Error".red(), e);
                        } else {
                            println!("{}", "✨ Zod schemas regenerated successfully!".green());
                            processor.stats.print_summary();
                        }
                    }
                }
            } else {
                let mut processor = FileProcessor::new();
                println!("{}", "Processing C# DTOs...".green());
                if let Err(e) =
                    process_single_file(&mut processor, &input_dir, &output_dir, &config)
                {
                    eprintln!("{}: {}", "Error".red(), e);
                    std::process::exit(1);
                }
                println!("{}", "✨ Zod schemas generated successfully!".green());
                processor.stats.print_summary();
            }
        }
    }
}
