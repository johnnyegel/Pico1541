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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackLayout {
    /// A standard GCR track that emulates real physical rotation.
    Accurate(u32),
    /// A GCR track that allows "warping" (skipping gaps) for performance.
    Warp(u32),
    /// An unformatted track (delivers 0x55 noise, no SYNC).
    Empty,
}

#[allow(async_fn_in_trait)]
pub trait DiskImage {
    /// Loads a specific half-track into the provided buffer.
    /// Returns the layout of the loaded track.
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error>;
}

pub struct VirtualDirImage<D: DirectorySource> {
    pub source: D,
    id: [u8; 2],
    pub sector1_idx: usize,
    pub sector2_idx: usize,
    pub next_sector_to_update: u8,
}

impl<D: DirectorySource> VirtualDirImage<D> {
    pub fn new(source: D) -> Self {
        Self {
            source,
            id: [0x56, 0x44], // "VD"
            sector1_idx: 0,
            sector2_idx: 0,
            next_sector_to_update: 2, 
        }
    }

    pub fn set_id(&mut self, id: [u8; 2]) {
        self.id = id;
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

    pub fn encode_directory_sector_gcr(&self, entries: &[crate::storage::DirectoryEntry], next_track: u8, next_sector: u8, dest_gcr: &mut [u8]) {
        let mut raw = [0u8; 256];
        self.encode_directory_sector(entries, next_track, next_sector, &mut raw);
        
        let mut data_checksum = 0u8;
        for b in raw.iter() { data_checksum ^= b; }
        let mut data_raw = [0u8; 260];
        data_raw[0] = 0x07;
        data_raw[1..257].copy_from_slice(&raw);
        data_raw[257] = data_checksum;

        for i in 0..65 {
            let mut src = [0u8; 4];
            src.copy_from_slice(&data_raw[i*4..(i+1)*4]);
            crate::gcr::encode_gcr_block(&src, (&mut dest_gcr[i*5..(i+1)*5]).try_into().unwrap());
        }
    }

    pub fn update_dynamic_buffer(&mut self, current_byte_idx: usize, gcr_data: &mut [u8]) -> bool {
        let sector = if current_byte_idx < 40 { 0xFF } else { ((current_byte_idx - 40) / 372) as u8 };
        
        let (target_sector, next_after) = if sector == 1 && self.next_sector_to_update == 2 {
            (2, 1)
        } else if sector == 2 && self.next_sector_to_update == 1 {
            (1, 2)
        } else {
            return false;
        };

        let idx = if target_sector == 1 { self.sector1_idx } else { self.sector2_idx };
        if idx == 0 { return false; }

        let mut entries = [crate::storage::DirectoryEntry::new(b"", 0, 0, 0); 8];
        let mut count = 0;
        for i in 0..8 {
            if let Some(entry) = self.source.next_entry() {
                entries[i] = entry;
                count += 1;
            } else { break; }
        }

        let (next_t, next_s) = if count == 0 {
            (0, 0xFF)
        } else {
            (18, if target_sector == 1 { 2 } else { 1 })
        };

        let mut dest_gcr = [0u8; 325];
        self.encode_directory_sector_gcr(&entries[..count], next_t, next_s, &mut dest_gcr);
        
        if idx + 325 <= gcr_data.len() {
            gcr_data[idx..idx+325].copy_from_slice(&dest_gcr);
            self.next_sector_to_update = next_after;
            return true;
        }
        false
    }
}

impl<D: DirectorySource> DiskImage for VirtualDirImage<D> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error> {
        if half_track % 2 != 0 {
            return Ok(TrackLayout::Empty);
        }

        let track = half_track / 2;
        if track != 18 {
            return Ok(TrackLayout::Empty);
        }

        // Track 18: Generate BAM and initial directory sectors
        let sectors = 19; 
        let mut write_ptr = 0;

        for _ in 0..40 { if write_ptr < dest.len() { dest[write_ptr] = 0x55; write_ptr += 1; } }

        self.source.rewind();

        for sector in 0..sectors {
            let mut sector_data = [0u8; 256];
            
            if sector == 0 {
                sector_data[0] = 18; sector_data[1] = 1; sector_data[2] = 0x41;
                for i in 4..144 { sector_data[i] = 0; }
                sector_data[144..160].copy_from_slice(self.source.name());
                sector_data[160..162].copy_from_slice(b"\xA0\xA0");
                sector_data[162..164].copy_from_slice(&self.id);
                sector_data[164] = 0xA0; sector_data[165] = 0x32; sector_data[166] = 0x41;
            } else if sector == 1 || sector == 2 {
                if sector == 1 { self.sector1_idx = write_ptr + 39; }
                else { self.sector2_idx = write_ptr + 39; }

                let mut entries = [crate::storage::DirectoryEntry::new(b"", 0, 0, 0); 8];
                let mut count = 0;
                for i in 0..8 {
                    if let Some(entry) = self.source.next_entry() {
                        entries[i] = entry;
                        count += 1;
                    } else { break; }
                }
                let (next_t, next_s) = if count == 0 {
                    (0, 0xFF)
                } else {
                    (18, if sector == 1 { 2 } else { 1 })
                };
                self.encode_directory_sector(&entries[..count], next_t, next_s, &mut sector_data);
            }

            for _ in 0..10 { if write_ptr < dest.len() { dest[write_ptr] = 0xFF; write_ptr += 1; } }
            let checksum = sector ^ track ^ self.id[0] ^ self.id[1];
            let header_raw = [0x08, checksum, sector, track, self.id[1], self.id[0], 0x0F, 0x0F];
            for i in 0..2 {
                let mut src = [0u8; 4]; src.copy_from_slice(&header_raw[i*4..(i+1)*4]);
                let mut dst = [0u8; 5]; gcr::encode_gcr_block(&src, &mut dst);
                if write_ptr + 5 <= dest.len() { dest[write_ptr..write_ptr+5].copy_from_slice(&dst); write_ptr += 5; }
            }
            for _ in 0..9 { if write_ptr < dest.len() { dest[write_ptr] = 0x55; write_ptr += 1; } }
            for _ in 0..10 { if write_ptr < dest.len() { dest[write_ptr] = 0xFF; write_ptr += 1; } }
            let mut data_checksum = 0u8;
            for b in sector_data.iter() { data_checksum ^= b; }
            let mut data_raw = [0u8; 260];
            data_raw[0] = 0x07; data_raw[1..257].copy_from_slice(&sector_data); data_raw[257] = data_checksum;
            for i in 0..65 {
                let mut src = [0u8; 4]; src.copy_from_slice(&data_raw[i*4..(i+1)*4]);
                let mut dst = [0u8; 5]; gcr::encode_gcr_block(&src, &mut dst);
                if write_ptr + 5 <= dest.len() { dest[write_ptr..write_ptr+5].copy_from_slice(&dst); write_ptr += 5; }
            }
            for _ in 0..8 { if write_ptr < dest.len() { dest[write_ptr] = 0x55; write_ptr += 1; } }
        }

        Ok(TrackLayout::Warp(57136))
    }
}

pub struct D64Image<S: Read + Seek> {
    storage: S,
    id: [u8; 2],
}

impl<S: Read + Seek> D64Image<S> {
    pub fn new(storage: S) -> Self {
        Self { storage, id: [0x31, 0x31] }
    }
    pub fn set_id(&mut self, id: [u8; 2]) { self.id = id; }
    fn get_sectors_per_track(track: u8) -> u8 {
        match track {
            1..=17 => 21, 18..=24 => 19, 25..=30 => 18, 31..=40 => 17, _ => 0,
        }
    }
    fn get_track_offset(track: u8) -> u32 {
        let mut offset = 0u32;
        for t in 1..track { offset += (Self::get_sectors_per_track(t) as u32) * 256; }
        offset
    }
}

impl<S: Read + Seek> DiskImage for D64Image<S> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error> {
        if half_track % 2 != 0 {
            return Ok(TrackLayout::Empty);
        }
        let track = half_track / 2;
        if track < 1 || track > 40 { return Err(Error::InvalidTrack); }
        let sectors = Self::get_sectors_per_track(track);
        let track_offset = Self::get_track_offset(track);
        self.storage.seek(SeekFrom::Start(track_offset as u64)).await.map_err(|_| Error::Io)?;
        let mut write_ptr = 0;
        for _ in 0..40 { if write_ptr < dest.len() { dest[write_ptr] = 0x55; write_ptr += 1; } }
        for sector in 0..sectors {
            let mut sector_data = [0u8; 256];
            self.storage.read_exact(&mut sector_data).await.map_err(|_| Error::Io)?;
            for _ in 0..10 { if write_ptr < dest.len() { dest[write_ptr] = 0xFF; write_ptr += 1; } }
            let checksum = sector ^ track ^ self.id[0] ^ self.id[1];
            let header_raw = [0x08, checksum, sector, track, self.id[1], self.id[0], 0x0F, 0x0F];
            for i in 0..2 {
                let mut src = [0u8; 4]; src.copy_from_slice(&header_raw[i*4..(i+1)*4]);
                let mut dst = [0u8; 5]; gcr::encode_gcr_block(&src, &mut dst);
                if write_ptr + 5 <= dest.len() { dest[write_ptr..write_ptr+5].copy_from_slice(&dst); write_ptr += 5; }
            }
            for _ in 0..9 { if write_ptr < dest.len() { dest[write_ptr] = 0x55; write_ptr += 1; } }
            for _ in 0..10 { if write_ptr < dest.len() { dest[write_ptr] = 0xFF; write_ptr += 1; } }
            let mut data_checksum = 0u8;
            for b in sector_data.iter() { data_checksum ^= b; }
            let mut data_raw = [0u8; 260];
            data_raw[0] = 0x07; data_raw[1..257].copy_from_slice(&sector_data); data_raw[257] = data_checksum;
            for i in 0..65 {
                let mut src = [0u8; 4]; src.copy_from_slice(&data_raw[i*4..(i+1)*4]);
                let mut dst = [0u8; 5]; gcr::encode_gcr_block(&src, &mut dst);
                if write_ptr + 5 <= dest.len() { dest[write_ptr..write_ptr+5].copy_from_slice(&dst); write_ptr += 5; }
            }
            for _ in 0..8 { if write_ptr < dest.len() { dest[write_ptr] = 0x55; write_ptr += 1; } }
        }
        let (_, nominal_bits) = match track {
            1..=17 => (7692, 61536), 18..=24 => (7142, 57136), 25..=30 => (6666, 53328), _ => (6250, 50000),
        };
        Ok(TrackLayout::Accurate(nominal_bits))
    }
}

pub struct G64Image<S: Read + Seek> { storage: S }
impl<S: Read + Seek> G64Image<S> { pub fn new(storage: S) -> Self { Self { storage } } }
impl<S: Read + Seek> DiskImage for G64Image<S> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error> {
        if half_track < 1 || half_track > 84 { return Err(Error::InvalidTrack); }
        let track_ptr_offset = 12 + (half_track as u64 - 1) * 4;
        self.storage.seek(SeekFrom::Start(track_ptr_offset)).await.map_err(|_| Error::Io)?;
        let mut ptr_bytes = [0u8; 4]; self.storage.read_exact(&mut ptr_bytes).await.map_err(|_| Error::Io)?;
        let track_offset = u32::from_le_bytes(ptr_bytes);
        if track_offset == 0 { return Ok(TrackLayout::Empty); }
        self.storage.seek(SeekFrom::Start(track_offset as u64)).await.map_err(|_| Error::Io)?;
        let mut len_bytes = [0u8; 2]; self.storage.read_exact(&mut len_bytes).await.map_err(|_| Error::Io)?;
        let track_len = u16::from_le_bytes(len_bytes) as usize;
        if track_len > dest.len() { return Err(Error::TrackTooLarge); }
        self.storage.read_exact(&mut dest[0..track_len]).await.map_err(|_| Error::Io)?;
        Ok(TrackLayout::Accurate((track_len as u32) * 8))
    }
}

pub enum AnyDiskImage<S: Read + Seek, D: DirectorySource> {
    D64(D64Image<S>),
    G64(G64Image<S>),
    VirtualDir(VirtualDirImage<D>),
}

impl<S: Read + Seek, D: DirectorySource> AnyDiskImage<S, D> {
    pub fn as_virtual_dir_mut(&mut self) -> Option<&mut VirtualDirImage<D>> {
        if let Self::VirtualDir(img) = self { Some(img) } else { None }
    }
}

impl<S: Read + Seek, D: DirectorySource> DiskImage for AnyDiskImage<S, D> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error> {
        match self {
            Self::D64(img) => img.read_track(half_track, dest).await,
            Self::G64(img) => img.read_track(half_track, dest).await,
            Self::VirtualDir(img) => img.read_track(half_track, dest).await,
        }
    }
}
