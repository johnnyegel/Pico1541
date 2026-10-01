const MEMORY_SIZE: usize = 32768;

/// Computes the 1541-II ROM checksum for an 8KB block matching the 6502 self-test loop at $EAC9.
fn compute_rom_8k_checksum(rom_8k: &[u8]) -> u8 {
    let mut reg_76: u8 = 0xE0;
    let mut acc: u8 = 0;
    let mut carry: u8 = 0;
    for _ in 0..0x20 {
        reg_76 = reg_76.wrapping_sub(1);
        let page_offset = (reg_76.wrapping_sub(0xC0) as usize) * 256;
        for y in 0..256 {
            let b = rom_8k[page_offset + y];
            let s = (acc as u16) + (b as u16) + (carry as u16);
            acc = (s & 0xFF) as u8;
            carry = (s > 0xFF) as u8;
        }
    }
    let s = (acc as u16) + (carry as u16);
    (s & 0xFF) as u8
}

pub struct Memory {
    data: &'static mut [u8; MEMORY_SIZE],
}

impl Memory {
    #[inline(always)]
    pub fn new(data: &'static mut [u8; MEMORY_SIZE]) -> Self {
        data.fill(0);
        Self { data }
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    pub fn read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            self.data[(addr & 0x07FF) as usize]
        } else {
            self.data[(addr & 0x7FFF) as usize]
        }
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    pub fn get_ptr(&self, addr: u16) -> *const u8 {
        if addr < 0x8000 {
            &self.data[(addr & 0x07FF) as usize] as *const u8
        } else {
            &self.data[(addr & 0x7FFF) as usize] as *const u8
        }
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    pub fn write(&mut self, addr: u16, val: u8) -> bool {
        if addr < 0x8000 {
            let index = (addr & 0x07FF) as usize;
            self.data[index] = val;

            // Return true if IEC listen/talk addresses were updated
            return index.wrapping_sub(0x0077) <= 1;
        }
        false
    }


    #[inline(always)]
    pub fn load_rom(&mut self, rom: &'static [u8; 16384]) {
        self.data[0x4000..0x8000].copy_from_slice(rom);

        // Extended 7-type directory tables at 6502 address $BF00 ($3F00 in self.data)
        // Indices: 0=DEL, 1=SEQ, 2=PRG, 3=USR, 4=REL, 5=CBM, 6=DIR
        self.data[0x3F00..0x3F08].copy_from_slice(b"DSPURCD\x00"); // ext_typlst ($BF00)
        self.data[0x3F08..0x3F10].copy_from_slice(b"EERSEBI\x00"); // ext_tp1lst ($BF08)
        self.data[0x3F10..0x3F18].copy_from_slice(b"LQGRLMR\x00"); // ext_tp2lst ($BF10)

        // Dynamically detect directory type formatting pattern in ROM (trnsfr.s):
        // LDA tp2,Y (B9 xx xx)
        // STA nambuf,X (9D xx xx)
        // DEX (CA)
        // LDA tp1,Y (B9 xx xx)
        // STA nambuf,X (9D xx xx)
        // DEX (CA)
        // LDA typ,Y (B9 xx xx)
        // STA nambuf,X (9D xx xx)
        let rom_slice = &mut self.data[0x4000..0x8000];
        let mut patched = false;
        let orig_chk = compute_rom_8k_checksum(&rom_slice[0..0x2000]);

        for i in 0..(rom_slice.len().saturating_sub(18)) {
            if rom_slice[i] == 0xB9
                && rom_slice[i + 3] == 0x9D
                && rom_slice[i + 6] == 0xCA
                && rom_slice[i + 7] == 0xB9
                && rom_slice[i + 10] == 0x9D
                && rom_slice[i + 13] == 0xCA
                && rom_slice[i + 14] == 0xB9
                && rom_slice[i + 17] == 0x9D
            {
                if rom_slice[i + 4..i + 6] == rom_slice[i + 11..i + 13]
                    && rom_slice[i + 4..i + 6] == rom_slice[i + 18..i + 20]
                {
                    // Repoint table lookups to our clean extension table at $BF00
                    rom_slice[i + 1] = 0x10;
                    rom_slice[i + 2] = 0xBF; // $BF10: ext_tp2lst
                    rom_slice[i + 8] = 0x08;
                    rom_slice[i + 9] = 0xBF; // $BF08: ext_tp1lst
                    rom_slice[i + 15] = 0x00;
                    rom_slice[i + 16] = 0xBF; // $BF00: ext_typlst
                    patched = true;
                    break;
                }
            }
        }

        // If the ROM was patched and is in the $C000..$DFFF block, adjust cchksm at rom_slice[0]
        // ($C000) so the 1541-II boot self-test checksum remains 100% valid.
        if patched {
            for cand in 0..=255 {
                rom_slice[0] = cand;
                if compute_rom_8k_checksum(&rom_slice[0..0x2000]) == orig_chk {
                    break;
                }
            }
        }
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.data[0..0x0800].fill(0);
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    pub fn get_iec_addresses(&self) -> (u8, u8) {
        (self.data[0x0077], self.data[0x0078])
    }
}

impl core::ops::Index<usize> for Memory {
    type Output = u8;
    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl core::ops::IndexMut<usize> for Memory {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}

impl core::ops::Index<core::ops::Range<usize>> for Memory {
    type Output = [u8];
    #[inline(always)]
    fn index(&self, index: core::ops::Range<usize>) -> &Self::Output {
        &self.data[index]
    }
}

impl core::ops::IndexMut<core::ops::Range<usize>> for Memory {
    #[inline(always)]
    fn index_mut(&mut self, index: core::ops::Range<usize>) -> &mut Self::Output {
        &mut self.data[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extended_directory_tables_and_rom_patch() {
        let mut sys_mem = [0u8; MEMORY_SIZE];
        let mut memory = Memory::new(unsafe {
            &mut *core::ptr::addr_of_mut!(sys_mem)
        });

        // Construct a synthetic 16KB ROM containing the 1541 DOS trnsfr routine pattern
        static mut TEST_ROM: [u8; 16384] = [0xEA; 16384];
        let rom = unsafe { &mut *core::ptr::addr_of_mut!(TEST_ROM) };
        let offset = 0x074E; // standard trnsfr offset in 1541-II
        // Opcode pattern:
        // LDA $FEC5,Y; STA $02B1,X; DEX
        rom[offset] = 0xB9; rom[offset + 1] = 0xC5; rom[offset + 2] = 0xFE;
        rom[offset + 3] = 0x9D; rom[offset + 4] = 0xB1; rom[offset + 5] = 0x02;
        rom[offset + 6] = 0xCA;
        // LDA $FEC0,Y; STA $02B1,X; DEX
        rom[offset + 7] = 0xB9; rom[offset + 8] = 0xC0; rom[offset + 9] = 0xFE;
        rom[offset + 10] = 0x9D; rom[offset + 11] = 0xB1; rom[offset + 12] = 0x02;
        rom[offset + 13] = 0xCA;
        // LDA $FEBB,Y; STA $02B1,X
        rom[offset + 14] = 0xB9; rom[offset + 15] = 0xBB; rom[offset + 16] = 0xFE;
        rom[offset + 17] = 0x9D; rom[offset + 18] = 0xB1; rom[offset + 19] = 0x02;

        memory.load_rom(rom);

        // Verify extended tables at $BF00
        assert_eq!(memory.read(0xBF00), b'D'); // DEL
        assert_eq!(memory.read(0xBF08), b'E');
        assert_eq!(memory.read(0xBF10), b'L');

        assert_eq!(memory.read(0xBF00 + 2), b'P'); // PRG
        assert_eq!(memory.read(0xBF08 + 2), b'R');
        assert_eq!(memory.read(0xBF10 + 2), b'G');

        // Type 6 is DIR
        assert_eq!(memory.read(0xBF00 + 6), b'D');
        assert_eq!(memory.read(0xBF08 + 6), b'I');
        assert_eq!(memory.read(0xBF10 + 6), b'R');

        // Verify ROM pattern was repointed to $BF10, $BF08, $BF00
        assert_eq!(memory.read(0xC000 + offset as u16 + 1), 0x10);
        assert_eq!(memory.read(0xC000 + offset as u16 + 2), 0xBF);
        assert_eq!(memory.read(0xC000 + offset as u16 + 8), 0x08);
        assert_eq!(memory.read(0xC000 + offset as u16 + 9), 0xBF);
        assert_eq!(memory.read(0xC000 + offset as u16 + 15), 0x00);
        assert_eq!(memory.read(0xC000 + offset as u16 + 16), 0xBF);
    }
}

