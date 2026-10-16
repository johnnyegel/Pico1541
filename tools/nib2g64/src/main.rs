use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use nib2g64::{convert_nib_to_g64, AlignMode, ConversionReport};

fn print_usage() {
    eprintln!("nib2g64 - Convert Commodore .nib and .nbz disk images to .g64 format");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("    nib2g64 [OPTIONS] <input-path>");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("    <input-path>           Path to .nib / .nbz file or directory containing disk images");
    eprintln!();
    eprintln!("Options:");
    eprintln!("    -o, --output <PATH>    Output .g64 file path (invalid with directory input)");
    eprintln!("        --keep-raw         Keep raw captured bits instead of zeroing weak bits");
    eprintln!("        --align <MODE>     Track alignment mode: auto, none, sync [default: auto]");
    eprintln!("    -v, --verbose          Enable verbose per-track reporting");
    eprintln!("    -h, --help             Print help information");
}

fn parse_align_mode(s: &str) -> Result<AlignMode, String> {
    match s.to_ascii_lowercase().as_str() {
        "auto" => Ok(AlignMode::Auto),
        "none" => Ok(AlignMode::None),
        "sync" => Ok(AlignMode::Sync),
        _ => Err(format!(
            "invalid align mode '{}' (expected auto, none, or sync)",
            s
        )),
    }
}

fn convert_single_file(
    input_path: &Path,
    output_path: &Path,
    keep_raw: bool,
    align_mode: AlignMode,
    verbose: bool,
) -> Result<(usize, ConversionReport), String> {
    if input_path == output_path {
        return Err(format!(
            "input and output paths are identical: {}",
            input_path.display()
        ));
    }

    if let (Ok(in_canon), Ok(out_canon)) = (input_path.canonicalize(), output_path.canonicalize()) {
        if in_canon == out_canon {
            return Err(format!(
                "input and output paths resolve to the same file: {}",
                input_path.display()
            ));
        }
    }

    let input_bytes = fs::read(input_path).map_err(|e| {
        format!("failed to read input file '{}': {}", input_path.display(), e)
    })?;

    let (g64_bytes, report) = convert_nib_to_g64(&input_bytes, keep_raw, align_mode, verbose)?;

    fs::write(output_path, &g64_bytes).map_err(|e| {
        format!("failed to write output file '{}': {}", output_path.display(), e)
    })?;

    Ok((g64_bytes.len(), report))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut keep_raw = false;
    let mut align_mode = AlignMode::Auto;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            "-v" | "--verbose" => {
                verbose = true;
            }
            "--keep-raw" => {
                keep_raw = true;
            }
            "-o" | "--output" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: missing argument for -o / --output");
                    print_usage();
                    process::exit(1);
                }
                output = Some(PathBuf::from(&args[i]));
            }
            "--align" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: missing argument for --align");
                    print_usage();
                    process::exit(1);
                }
                match parse_align_mode(&args[i]) {
                    Ok(mode) => align_mode = mode,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        process::exit(1);
                    }
                }
            }
            _ => {
                if let Some(val) = arg.strip_prefix("-o=") {
                    output = Some(PathBuf::from(val));
                } else if let Some(val) = arg.strip_prefix("--output=") {
                    output = Some(PathBuf::from(val));
                } else if let Some(val) = arg.strip_prefix("--align=") {
                    match parse_align_mode(val) {
                        Ok(mode) => align_mode = mode,
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            process::exit(1);
                        }
                    }
                } else if arg.starts_with('-') {
                    eprintln!("Error: unknown option '{}'", arg);
                    print_usage();
                    process::exit(1);
                } else if input.is_none() {
                    input = Some(PathBuf::from(arg));
                } else {
                    eprintln!("Error: unexpected positional argument '{}'", arg);
                    print_usage();
                    process::exit(1);
                }
            }
        }
        i += 1;
    }

    let Some(input_path) = input else {
        eprintln!("Error: missing input file or directory");
        print_usage();
        process::exit(1);
    };

    if !input_path.exists() {
        eprintln!("Error: input path '{}' does not exist", input_path.display());
        process::exit(1);
    }

    if input_path.is_dir() {
        if output.is_some() {
            eprintln!("Error: -o / --output cannot be specified when converting a directory");
            process::exit(1);
        }

        let entries = match fs::read_dir(&input_path) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("Error reading directory '{}': {}", input_path.display(), e);
                process::exit(1);
            }
        };

        let mut files_to_convert = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Warning: could not read entry: {}", e);
                    continue;
                }
            };
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy();
                    if ext_str.eq_ignore_ascii_case("nib") || ext_str.eq_ignore_ascii_case("nbz") {
                        files_to_convert.push(path);
                    }
                }
            }
        }

        files_to_convert.sort();

        if files_to_convert.is_empty() {
            println!(
                "No .nib or .nbz files found in directory '{}'",
                input_path.display()
            );
            println!("Batch conversion completed: 0 succeeded, 0 failed");
            return;
        }

        let mut succeeded = 0;
        let mut failed = 0;

        for path in &files_to_convert {
            let out_path = path.with_extension("g64");
            println!("Converting: {} -> {}", path.display(), out_path.display());
            match convert_single_file(path, &out_path, keep_raw, align_mode, verbose) {
                Ok((size, report)) => {
                    succeeded += 1;
                    let weak_label = if keep_raw {
                        "weak bytes detected"
                    } else {
                        "weak bytes sanitized"
                    };
                    println!(
                        "  Success: {} tracks, {} {}, {} bytes written",
                        report.tracks_converted, report.weak_bytes_sanitized, weak_label, size
                    );
                }
                Err(e) => {
                    failed += 1;
                    eprintln!("  Failed converting {}: {}", path.display(), e);
                }
            }
        }

        println!(
            "Batch conversion completed: {} succeeded, {} failed",
            succeeded, failed
        );

        if failed > 0 {
            process::exit(1);
        }
    } else if input_path.is_file() {
        let output_path = output.unwrap_or_else(|| input_path.with_extension("g64"));
        match convert_single_file(&input_path, &output_path, keep_raw, align_mode, verbose) {
            Ok((size, report)) => {
                let weak_label = if keep_raw {
                    "weak bytes detected"
                } else {
                    "weak bytes sanitized"
                };
                println!(
                    "Converted {} -> {}: {} tracks processed, {} {}, {} bytes written",
                    input_path.display(),
                    output_path.display(),
                    report.tracks_converted,
                    report.weak_bytes_sanitized,
                    weak_label,
                    size
                );
            }
            Err(e) => {
                eprintln!("Error converting {}: {}", input_path.display(), e);
                process::exit(1);
            }
        }
    } else {
        eprintln!(
            "Error: input path '{}' is neither a regular file nor a directory",
            input_path.display()
        );
        process::exit(1);
    }
}
