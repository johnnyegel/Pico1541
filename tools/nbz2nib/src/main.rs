mod lz77;

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn convert_file(input_path: &Path, output_path: &Path) -> Result<(), Box<dyn Error>> {
    if input_path == output_path {
        return Err(format!(
            "Input and output paths are identical: {}",
            input_path.display()
        )
        .into());
    }

    if let (Ok(in_canon), Ok(out_canon)) = (input_path.canonicalize(), output_path.canonicalize()) {
        if in_canon == out_canon {
            return Err(format!(
                "Input and output paths resolve to the same file: {}",
                input_path.display()
            )
            .into());
        }
    }

    println!(
        "Decompressing: {} -> {}",
        input_path.display(),
        output_path.display()
    );
    let data = fs::read(input_path)?;
    let decompressed = lz77::decompress(&data)?;

    if decompressed.len() < 0x100 {
        return Err("Decompressed file too small to contain a valid NIB header".into());
    }

    let magic = &decompressed[0..13];
    if magic != b"MNIB-1541-RAW" && magic != b"MNIB-1571-RAW" {
        return Err(format!(
            "Invalid NIB magic signature: {:?}",
            String::from_utf8_lossy(magic)
        )
        .into());
    }

    #[allow(clippy::chunks_exact_to_as_chunks)]
    let track_count = decompressed[0x10..0x100]
        .chunks_exact(2)
        .take(84)
        .take_while(|pair| pair[0] != 0)
        .count();

    let expected_min_len = 0x100 + track_count * 8192;
    if decompressed.len() < expected_min_len {
        return Err(format!(
            "Decompressed file is truncated: expected at least {} bytes for {} tracks, got {} bytes",
            expected_min_len,
            track_count,
            decompressed.len()
        )
        .into());
    }

    println!(
        "Decompressed {} bytes, {} tracks",
        decompressed.len(),
        track_count
    );

    fs::write(output_path, decompressed)?;
    println!("Successfully wrote {}", output_path.display());
    Ok(())
}

fn print_usage() {
    eprintln!("Usage: nbz2nib <input.nbz> [-o <output.nib> | <output.nib>]");
    eprintln!("       nbz2nib <directory_with_nbz_files>");
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            "-o" | "--output" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: missing argument for -o");
                    print_usage();
                    process::exit(1);
                }
                output = Some(PathBuf::from(&args[i]));
            }
            arg => {
                if input.is_none() {
                    input = Some(PathBuf::from(arg));
                } else if output.is_none() {
                    output = Some(PathBuf::from(arg));
                } else {
                    eprintln!("Error: unexpected argument '{}'", arg);
                    print_usage();
                    process::exit(1);
                }
            }
        }
        i += 1;
    }

    let Some(input_path) = input else {
        print_usage();
        process::exit(1);
    };

    if input_path.is_dir() {
        if output.is_some() {
            eprintln!("Error: -o / output path cannot be specified when converting a directory");
            process::exit(1);
        }
        let mut success_count = 0;
        let mut failure_count = 0;
        for entry in fs::read_dir(&input_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("nbz") {
                        let out_path = path.with_extension("nib");
                        match convert_file(&path, &out_path) {
                            Ok(()) => success_count += 1,
                            Err(e) => {
                                eprintln!("Error converting {}: {}", path.display(), e);
                                failure_count += 1;
                            }
                        }
                    }
                }
            }
        }
        println!(
            "Batch conversion completed: {} succeeded, {} failed",
            success_count, failure_count
        );
        if failure_count > 0 {
            process::exit(1);
        }
    } else {
        let output_path = output.unwrap_or_else(|| input_path.with_extension("nib"));
        convert_file(&input_path, &output_path)?;
    }

    Ok(())
}
