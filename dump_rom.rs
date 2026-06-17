use std::fs;
fn main() {
    let rom = fs::read("docs/dos1541-master/dos.bin").unwrap();
    let addr = 0xE876 - 0xC000;
    println!("{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}", 
             rom[addr], rom[addr+1], rom[addr+2], rom[addr+3], rom[addr+4], rom[addr+5], rom[addr+6], rom[addr+7]);
}
