mod lz77;

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn convert_file(input_path: &Path, output_path: &Path) -> Result<(), Box<dyn Error>> {
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

    fs::write(output_path, decompressed)?;
    println!("Successfully wrote {}", output_path.display());
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: nbz2nib <input.nbz> [output.nib]");
        eprintln!("       nbz2nib <directory_with_nbz_files>");
        process::exit(1);
    }

    let input_path = Path::new(&args[1]);
    if input_path.is_dir() {
        for entry in fs::read_dir(input_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("nbz") {
                        let out_path = path.with_extension("nib");
                        if let Err(e) = convert_file(&path, &out_path) {
                            eprintln!("Error converting {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
    } else {
        let output_path = if args.len() >= 3 {
            PathBuf::from(&args[2])
        } else {
            input_path.with_extension("nib")
        };
        convert_file(input_path, &output_path)?;
    }

    Ok(())
}
