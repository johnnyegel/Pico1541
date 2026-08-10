#![no_std]

pub mod emulation;
pub mod magnetic_surface;
pub mod pio {
    pub fn magnetic_surface_program() -> ::pio::Program<32> {
        ::pio::pio_file!("src/pio/magnetic_surface.pio").program
    }
}
