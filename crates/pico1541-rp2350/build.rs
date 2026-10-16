use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=../../flash-fs");
    println!("cargo::rustc-check-cfg=cfg(has_mklittlefs)");

    // Check if mklittlefs is available
    let mklittlefs_status = std::process::Command::new("mklittlefs")
        .arg("--version")
        .output();

    if mklittlefs_status.is_ok() {
        // Run mklittlefs
        let fs_bin = out.join("fs.bin");
        let status = std::process::Command::new("mklittlefs")
            .args([
                "-c",
                "../../flash-fs",
                "-s",
                "2097152", // 2048KB (2MB) for RP2350 Pico 2
                "-p",
                "256", // FLASH_PAGE_SIZE
                "-b",
                "4096", // FLASH_SECTOR_SIZE
                fs_bin.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to execute mklittlefs");

        if status.success() {
            println!("cargo:rustc-cfg=has_mklittlefs");
        } else {
            panic!(
                "\n\n[ERROR] mklittlefs execution failed!\n\
                The files in 'flash-fs/' exceed the 2 MB partition limit.\n\
                Please remove unneeded files from 'flash-fs/files/' or move them to an SD card.\n\n"
            );
        }
    } else {
        println!("cargo:warning=mklittlefs tool is not available. An empty image will be initialized on boot instead.");
    }
}
