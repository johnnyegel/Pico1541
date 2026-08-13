const MEMORY_SIZE: usize = 32768;

pub struct Memory {
    data: &'static mut [u8; MEMORY_SIZE],
}

impl Memory {
    #[inline(always)]
    pub fn new(data: &'static mut [u8; MEMORY_SIZE]) -> Self {
        data.fill(0);
        Self { data }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            let index = (addr & 0x07FF) as usize;
            self.data[index]
        } else {
            let index = ((addr & 0x3FFF) | ((addr >> 1) & 0x4000)) as usize;
            self.data[index]
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn get_ptr(&self, addr: u16) -> *const u8 {
        if addr < 0x8000 {
            let index = (addr & 0x07FF) as usize;
            &self.data[index] as *const u8
        } else {
            let index = ((addr & 0x3FFF) | ((addr >> 1) & 0x4000)) as usize;
            &self.data[index] as *const u8
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn write(&mut self, addr: u16, val: u8) -> bool {
        if addr < 0x8000 {
            let index = (addr & 0x07FF) as usize;
            self.data[index] = val;
            
            // Return true if IEC listen/talk addresses were updated
            return index == 0x0077 || index == 0x0078;
        }
        false
    }

    #[inline(always)]
    pub fn load_rom(&mut self, rom: &'static [u8; 16384]) {
        self.data[0x4000..0x8000].copy_from_slice(rom);
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.data[0..0x0800].fill(0);
    }

    #[link_section = ".data.ram_func"]
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
