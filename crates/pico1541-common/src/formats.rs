use crate::gcr;
use crate::storage::DirectorySource;
use embedded_io_async::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub enum Error {
    Io,
    InvalidFormat,
    InvalidTrack,
    TrackTooLarge,
}

#[allow(async_fn_in_trait)]
pub trait DiskImage {
    /// Loads a specific half-track into the provided buffer.
    /// Returns the number of bits in the track.
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, Error>;
}

pub struct VirtualDirImage<D: DirectorySource> {
    pub source: D,
    id: [u8; 2],
    pub sector1_idx: usize,
    pub sector2_idx: usize,
}

impl<D: DirectorySource> VirtualDirImage<D> {
    pub fn new(source: D) -> Self {
        Self {
            source,
            id: [0x56, 0x44], // "VD"
            sector1_idx: 0,
            sector2_idx: 0,
        }
    }

    pub fn encode_directory_sector(&self, entries: &[crate::storage::DirectoryEntry], next_track: u8, next_sector: u8, dest: &mut [u8]) {
        let mut raw = [0u8; 256];
        raw[0] = next_track;
        raw[1] = next_sector;
        
        for (i, entry) in entries.iter().enumerate() {
            let offset = 2 + i * 32;
            if offset + 32 > 256 { break; }
            raw[offset] = entry.file_type;
            raw[offset+1] = entry.first_track;
            raw[offset+2] = entry.first_sector;
            raw[offset+3..offset+19].copy_from_slice(&entry.filename);
            raw[offset+19] = 0; // side track
            raw[offset+20] = 0; // side sector
            raw[offset+21] = 0; // REL record length
            // 22-25 unused
            // 26-27 unused
            raw[offset+28] = (entry.block_size & 0xFF) as u8;
            raw[offset+29] = (entry.block_size >> 8) as u8;
        }
        dest.copy_from_slice(&raw);
    }
}

impl<D: DirectorySource> DiskImage for VirtualDirImage<D> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, Error> {
        if half_track % 2 != 0 {
            dest.fill(0x55);
            return Ok(35000);
        }

        let track = half_track / 2;
        if track != 18 {
            // Empty track for non-18
            dest.fill(0x55);
            return Ok(match track {
                1..=17 => 61536,
                18..=24 => 57136,
                25..=30 => 53328,
                _ => 50000,
            });
        }

        // Track 18: Generate BAM and initial directory sectors
        let sectors = 19; // Track 18 has 19 sectors
        let mut write_ptr = 0;

        // Lead-in gap
        for _ in 0..40 {
            if write_ptr < dest.len() {
                dest[write_ptr] = 0x55;
                write_ptr += 1;
            }
        }

        self.source.rewind();

        for sector in 0..sectors {
            let mut sector_data = [0u8; 256];
            
            if sector == 0 {
                // BAM Sector
                sector_data[0] = 18; // Next track (18)
                sector_data[1] = 1;  // Next sector (1)
                sector_data[2] = 0x41; // DOS version 'A'
                // ... Fill rest with dummy BAM info
                for i in 4..144 { sector_data[i] = 0; }
                sector_data[144..160].copy_from_slice(self.source.name());
                sector_data[160..162].copy_from_slice(b"\xA0\xA0");
                sector_data[162..164].copy_from_slice(&self.id);
                sector_data[164] = 0xA0;
                sector_data[165] = 0x32; // '2'
                sector_data[166] = 0x41; // 'A'
            } else if sector == 1 || sector == 2 {
                // We record where the DATA GCR block starts for this sector
                // write_ptr is at the beginning of Header SYNC
                // Header SYNC (10) + Header GCR (10) + Gap (9) + Data SYNC (10) = 39
                if sector == 1 { self.sector1_idx = write_ptr + 39; }
                else { self.sector2_idx = write_ptr + 39; }

                let mut entries = [crate::storage::DirectoryEntry::new(b"", 0, 0, 0); 8];
                let mut count = 0;
                for i in 0..8 {
                    if let Some(entry) = self.source.next_entry() {
                        entries[i] = entry;
                        count += 1;
                    } else {
                        break;
                    }
                }
                
                // Link S1 to S2, and S2 to S1 (looping)
                let next_s = if sector == 1 { 2 } else { 1 };
                self.encode_directory_sector(&entries[..count], 18, next_s, &mut sector_data);
            }

            // Write GCR Sector
            // Header SYNC (10)
            for _ in 0..10 { if write_ptr < dest.len() { dest[write_ptr] = 0xFF; write_ptr += 1; } }
            
            let checksum = sector ^ track ^ self.id[0] ^ self.id[1];
            let header_raw = [0x08, checksum, sector, track, self.id[1], self.id[0], 0x0F, 0x0F];
            for i in 0..2 {
                let mut src = [0u8; 4];
                src.copy_from_slice(&header_raw[i*4..(i+1)*4]);
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(&src, &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr+5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }

            // Gap (9)
            for _ in 0..9 { if write_ptr < dest.len() { dest[write_ptr] = 0x55; write_ptr += 1; } }

            // Data SYNC (10)
            for _ in 0..10 { if write_ptr < dest.len() { dest[write_ptr] = 0xFF; write_ptr += 1; } }

            // Data GCR (325 bytes)
            let mut data_checksum = 0u8;
            for b in sector_data.iter() { data_checksum ^= b; }
            let mut data_raw = [0u8; 260];
            data_raw[0] = 0x07;
            data_raw[1..257].copy_from_slice(&sector_data);
            data_raw[257] = data_checksum;
            
            for i in 0..65 {
                let mut src = [0u8; 4];
                src.copy_from_slice(&data_raw[i*4..(i+1)*4]);
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(&src, &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr+5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }

            // Gap (8)
            for _ in 0..8 { if write_ptr < dest.len() { dest[write_ptr] = 0x55; write_ptr += 1; } }
        }

        Ok(57136) // Track 18 is Zone 2
    }
}

pub struct D64Image<S: Read + Seek> {
    storage: S,
    id: [u8; 2], // Disk ID for header generation
}

impl<S: Read + Seek> D64Image<S> {
    pub fn new(storage: S) -> Self {
        Self {
            storage,
            id: [0x31, 0x31], // Default ID "11"
        }
    }

    pub fn set_id(&mut self, id: [u8; 2]) {
        self.id = id;
    }

    fn get_sectors_per_track(track: u8) -> u8 {
        match track {
            1..=17 => 21,
            18..=24 => 19,
            25..=30 => 18,
            31..=40 => 17,
            _ => 0,
        }
    }

    fn get_track_offset(track: u8) -> u32 {
        let mut offset = 0u32;
        for t in 1..track {
            offset += (Self::get_sectors_per_track(t) as u32) * 256;
        }
        offset
    }
}

impl<S: Read + Seek> DiskImage for D64Image<S> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, Error> {
        if half_track % 2 != 0 {
            dest.fill(0x55);
            return Ok(35000);
        }

        let track = half_track / 2;
        if track < 1 || track > 40 {
            return Err(Error::InvalidTrack);
        }

        let sectors = Self::get_sectors_per_track(track);
        let track_offset = Self::get_track_offset(track);

        self.storage.seek(SeekFrom::Start(track_offset as u64)).await.map_err(|_| Error::Io)?;

        let mut write_ptr = 0;
        
        // Lead-in gap for the track
        for _ in 0..40 {
            if write_ptr < dest.len() {
                dest[write_ptr] = 0x55;
                write_ptr += 1;
            }
        }

        for sector in 0..sectors {
            let mut sector_data = [0u8; 256];
            self.storage.read_exact(&mut sector_data).await.map_err(|_| Error::Io)?;

            // Header SYNC (Standard ~10 bytes)
            for _ in 0..10 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }

            let checksum = sector ^ track ^ self.id[0] ^ self.id[1];
            let header_raw = [0x08, checksum, sector, track, self.id[1], self.id[0], 0x0F, 0x0F];
            
            for i in 0..2 {
                let mut src = [0u8; 4];
                src.copy_from_slice(&header_raw[i*4..(i+1)*4]);
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(&src, &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr+5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }

            // Header-to-Data Gap (Standard ~9 bytes)
            for _ in 0..9 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0x55;
                    write_ptr += 1;
                }
            }

            // Data SYNC (Standard ~10 bytes)
            for _ in 0..10 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }

            let mut data_checksum = 0u8;
            for b in sector_data.iter() {
                data_checksum ^= b;
            }
            
            let mut data_raw = [0u8; 260];
            data_raw[0] = 0x07;
            data_raw[1..257].copy_from_slice(&sector_data);
            data_raw[257] = data_checksum;
            data_raw[258] = 0x00;
            data_raw[259] = 0x00;

            for i in 0..65 {
                let mut src = [0u8; 4];
                src.copy_from_slice(&data_raw[i*4..(i+1)*4]);
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(&src, &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr+5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }

            // Inter-sector gap (Standard ~8 bytes)
            for _ in 0..8 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0x55;
                    write_ptr += 1;
                }
            }
        }

        let (nominal_bytes, nominal_bits) = match track {
            1..=17 => (7692, 61536),  // Zone 3: ~7692 bytes
            18..=24 => (7142, 57136), // Zone 2: ~7142 bytes
            25..=30 => (6666, 53328), // Zone 1: ~6666 bytes
            _ => (6250, 50000),       // Zone 0: ~6250 bytes
        };

        while write_ptr < nominal_bytes && write_ptr < dest.len() {
            dest[write_ptr] = 0x55;
            write_ptr += 1;
        }

        Ok(nominal_bits)
    }
}

pub struct G64Image<S: Read + Seek> {
    storage: S,
}

impl<S: Read + Seek> G64Image<S> {
    pub fn new(storage: S) -> Self {
        Self { storage }
    }
}

impl<S: Read + Seek> DiskImage for G64Image<S> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, Error> {
        if half_track < 1 || half_track > 84 {
            return Err(Error::InvalidTrack);
        }

        let track_ptr_offset = 12 + (half_track as u64 - 1) * 4;
        
        self.storage.seek(SeekFrom::Start(track_ptr_offset)).await.map_err(|_| Error::Io)?;
        let mut ptr_bytes = [0u8; 4];
        self.storage.read_exact(&mut ptr_bytes).await.map_err(|_| Error::Io)?;
        let track_offset = u32::from_le_bytes(ptr_bytes);

        if track_offset == 0 {
            dest.fill(0x55);
            return Ok(35000);
        }

        self.storage.seek(SeekFrom::Start(track_offset as u64)).await.map_err(|_| Error::Io)?;
        let mut len_bytes = [0u8; 2];
        self.storage.read_exact(&mut len_bytes).await.map_err(|_| Error::Io)?;
        let track_len = u16::from_le_bytes(len_bytes) as usize;

        if track_len > dest.len() {
            return Err(Error::TrackTooLarge);
        }

        self.storage.read_exact(&mut dest[0..track_len]).await.map_err(|_| Error::Io)?;

        Ok((track_len as u32) * 8)
    }
}

/// An enum-based wrapper for DiskImage to allow dynamic selection without dyn trait in no_std.
pub enum AnyDiskImage<S: Read + Seek, D: DirectorySource> {
    D64(D64Image<S>),
    G64(G64Image<S>),
    VirtualDir(VirtualDirImage<D>),
}

impl<S: Read + Seek, D: DirectorySource> AnyDiskImage<S, D> {
    pub fn as_virtual_dir_mut(&mut self) -> Option<&mut VirtualDirImage<D>> {
        if let Self::VirtualDir(img) = self {
            Some(img)
        } else {
            None
        }
    }
}

impl<S: Read + Seek, D: DirectorySource> DiskImage for AnyDiskImage<S, D> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, Error> {
        match self {
            Self::D64(img) => img.read_track(half_track, dest).await,
            Self::G64(img) => img.read_track(half_track, dest).await,
            Self::VirtualDir(img) => img.read_track(half_track, dest).await,
        }
    }
}
