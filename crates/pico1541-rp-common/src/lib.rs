#![no_std]

pub mod driver;
pub mod emulation;
pub mod flash_storage;
pub mod littlefs_storage;
pub mod storage_manager;

pub mod pio {
    pub fn magnetic_surface_program() -> ::pio::Program<32> {
        ::pio::pio_file!("pio/magnetic_surface.pio").program
    }
}
