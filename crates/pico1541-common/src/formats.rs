use crate::gcr;
use crate::storage::DirectorySource;
use embedded_io_async::{Read, Seek, SeekFrom};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    Io,
    InvalidFormat,
    InvalidTrack,
    TrackTooLarge,
    FileNotFound,
}

// Represents the layout of a track read from a disk image
// Accurate: A standard GCR track that emulates real physical rotation. Track length in bytes is provided.
// Warp: A GCR track that allows "warping", aka, the byte position does not require accurate updates
// StriddenWarp: A GCR track where sectors are at fixed strides, allowing instant warping via sector snooping.
// Empty: An unformatted track (delivers 0x55 noise, no SYNC).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackLayout {
    /// A standard GCR track that emulates real physical rotation.
    Accurate(usize),
    /// A GCR track that allows "warping" (skipping gaps) for performance.
    Warp(usize),
    /// A GCR track where sectors are at fixed strides, allowing instant warping via sector snooping.
    /// (total_size, sector_stride)
    StriddenWarp(usize, usize),
    /// There is no readable nor writable track (either no disk, or track is not formatted)
    /// This track layout can NOT be written to!
    Void,
    /// A special track layout that continuously emits SYNC bytes to pause the 1541 ROM.
    SyncWait,
}


#[allow(async_fn_in_trait)]
pub trait DiskImage {
    /// Loads a specific half-track into the provided buffer.
    /// Returns the layout of the loaded track.
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error>;
}

pub struct VirtualDiskImage<S, D: DirectorySource> {
    pub source: D,
    pub file_storage: Option<S>,
    id: [u8; 2],
    pub sector1_idx: usize,
    pub sector2_idx: usize,
    pub next_sector_to_update: u8,
    pub shortcut_entry: Option<crate::storage::DirectoryEntry>,
}

impl<S, D: DirectorySource> VirtualDiskImage<S, D> {
    pub fn new(source: D, name: &[u8]) -> Self {
        // Generate a pseudo-random ID based on the disk name, ensuring valid uppercase PETSCII (0x41-0x5A)
        let mut hash1 = 0u32;
        let mut hash2 = 0u32;
        for (i, &b) in name.iter().enumerate() {
            if i % 2 == 0 {
                hash1 = hash1.wrapping_add(b as u32);
            } else {
                hash2 = hash2.wrapping_add(b as u32);
            }
        }
        let id_high = 0x41 + (hash1 % 26) as u8;
        let id_low = 0x41 + (hash2 % 26) as u8;

        Self {
            source,
            file_storage: None,
            id: [id_high, id_low],
            sector1_idx: 0,
            sector2_idx: 0,
            next_sector_to_update: 1,
            shortcut_entry: None,
        }
    }

    pub fn set_id(&mut self, id: [u8; 2]) {
        self.id = id;
    }

    pub fn set_shortcut_entry(&mut self, entry: Option<crate::storage::DirectoryEntry>) {
        self.shortcut_entry = entry;
    }

    pub fn encode_directory_sector(
        &self,
        entries: &[crate::storage::DirectoryEntry],
        next_track: u8,
        next_sector: u8,
        dest: &mut [u8],
    ) {
        let mut raw = [0u8; 256];
        raw[0] = next_track;
        raw[1] = next_sector;

        for (i, entry) in entries.iter().enumerate() {
            let offset = 2 + i * 32;
            if offset + 32 > 256 {
                break;
            }
            raw[offset] = entry.file_type;
            raw[offset + 1] = entry.first_track;
            raw[offset + 2] = entry.first_sector;
            raw[offset + 3..offset + 19].copy_from_slice(&entry.filename);
            raw[offset + 19] = 0; // side track
            raw[offset + 20] = 0; // side sector
            raw[offset + 21] = 0; // REL record length
                                  // 22-25 unused
                                  // 26-27 unused
            raw[offset + 28] = (entry.block_size & 0xFF) as u8;
            raw[offset + 29] = (entry.block_size >> 8) as u8;
        }
        dest.copy_from_slice(&raw);
    }

    pub fn encode_directory_sector_gcr(
        &self,
        entries: &[crate::storage::DirectoryEntry],
        next_track: u8,
        next_sector: u8,
        dest_gcr: &mut [u8],
    ) {
        let mut raw = [0u8; 256];
        self.encode_directory_sector(entries, next_track, next_sector, &mut raw);

        let mut data_checksum = 0u8;
        for b in raw.iter() {
            data_checksum ^= b;
        }
        let mut data_raw = [0u8; 260];
        data_raw[0] = 0x07;
        data_raw[1..257].copy_from_slice(&raw);
        data_raw[257] = data_checksum;

        for (i, chunk) in data_raw.chunks_exact(4).enumerate() {
            crate::gcr::encode_gcr_block(
                chunk.try_into().unwrap(),
                (&mut dest_gcr[i * 5..(i + 1) * 5]).try_into().unwrap(),
            );
        }
    }

    pub fn update_dynamic_buffer(&mut self, current_byte_idx: usize, gcr_data: &mut [u8]) -> bool {
        let sector = if current_byte_idx < 40 {
            0xFF
        } else {
            ((current_byte_idx - 40) / 372) as u8
        };

        let (target_sector, next_after) = if sector == 1 && self.next_sector_to_update == 2 {
            (2, 1)
        } else if sector == 2 && self.next_sector_to_update == 1 {
            (1, 2)
        } else {
            return false;
        };

        let idx = if target_sector == 1 {
            self.sector1_idx
        } else {
            self.sector2_idx
        };
        if idx == 0 {
            return false;
        }

        let mut entries = [crate::storage::DirectoryEntry::new(b"", 0, 0, 0); 8];
        let mut count = 0;
        
        if let Some(entry) = self.shortcut_entry {
            if target_sector == 1 {
                entries[0] = entry;
                count = 1;
            } else {
                // Sector 2 is empty/end when shortcut is present
                count = 0;
            }
        } else {
            for entry_slot in &mut entries {
                if let Some(entry) = self.source.next_entry() {
                    *entry_slot = entry;
                    count += 1;
                } else {
                    break;
                }
            }
            let is_exhausted = self.source.as_paged_mut().map(|p| p.is_exhausted()).unwrap_or(true);
            if count == 0 && !is_exhausted {
                // Blocks not loaded yet by prefetcher, wait for next tick
                return false;
            }
        }

        let (next_t, next_s) = if count == 0 {
            (0, 0xFF)
        } else {
            (18, if target_sector == 1 { 2 } else { 1 })
        };

        let mut dest_gcr = [0u8; 325];
        self.encode_directory_sector_gcr(&entries[..count], next_t, next_s, &mut dest_gcr);

        if idx + 325 <= gcr_data.len() {
            gcr_data[idx..idx + 325].copy_from_slice(&dest_gcr);
            self.next_sector_to_update = next_after;
            return true;
        }
        false
    }

    pub fn force_update_directory_sectors(&mut self, gcr_data: &mut [u8]) {
        // To update Sector 1, we must simulate the head being in Sector 2
        self.next_sector_to_update = 1;
        self.update_dynamic_buffer(40 + 372 * 2 + 1, gcr_data); // Head in Sector 2, updates Sector 1
        
        // To update Sector 2, we must simulate the head being in Sector 1
        self.next_sector_to_update = 2;
        self.update_dynamic_buffer(40 + 372 + 1, gcr_data); // Head in Sector 1, updates Sector 2
        
        self.next_sector_to_update = 1; // When actual read starts, wait for Sector 1 to pass, then update Sector 2
    }
}

impl<S: Read + Seek, D: DirectorySource> DiskImage for VirtualDiskImage<S, D> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error> {
        if half_track > 83 {
            return Ok(TrackLayout::Void);
        }

        // Ensure no old bytes are left behind when the track is completely regenerated
        dest.fill(0x55);

        let track = half_track / 2 + 1;

        if track == 1 {
            // Track 1: Infinite File Track
            let sectors = 25;
            let stride = 400;

            for sector in 0..sectors {
                let mut write_ptr = sector * stride;

                // Lead-in
                for _ in 0..5 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0xAA;
                        write_ptr += 1;
                    }
                }

                // Header SYNC
                for _ in 0..10 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0xFF;
                        write_ptr += 1;
                    }
                }
                let checksum = (sector as u8) ^ track ^ self.id[0] ^ self.id[1];
                let header_raw = [
                    0x08, checksum, sector as u8, track, self.id[1], self.id[0], 0x0F, 0x0F,
                ];
                for chunk in header_raw.chunks_exact(4) {
                    let mut dst = [0u8; 5];
                    gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                    if write_ptr + 5 <= dest.len() {
                        dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                        write_ptr += 5;
                    }
                }
                // Header-to-Data Gap
                for _ in 0..9 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0xAA;
                        write_ptr += 1;
                    }
                }
                // Data SYNC
                for _ in 0..10 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0xFF;
                        write_ptr += 1;
                    }
                }
                
                let mut sector_data = [0u8; 256];
                if let Some(storage) = &mut self.file_storage {
                    let file_offset = (sector as u64) * 256;
                    let _ = storage.seek(SeekFrom::Start(file_offset)).await;
                    let _ = storage.read_exact(&mut sector_data).await;
                }

                let mut data_checksum = 0u8;
                for b in sector_data.iter() {
                    data_checksum ^= b;
                }

                let mut data_raw = [0u8; 260];
                data_raw[0] = 0x07;
                data_raw[1..257].copy_from_slice(&sector_data);
                data_raw[257] = data_checksum;

                for chunk in data_raw.chunks_exact(4) {
                    let mut dst = [0u8; 5];
                    gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                    if write_ptr + 5 <= dest.len() {
                        dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                        write_ptr += 5;
                    }
                }
            }

            log::info!("VirtualDiskImage: Returning StriddenWarp for track 1");
            return Ok(TrackLayout::StriddenWarp(10000, stride));
        }

        if half_track == 34 || half_track == 35 {
            // Track 18: Generate BAM and initial directory sectors
            let sectors = 19;
            let mut write_ptr = 0;

            for _ in 0..40 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xAA;
                    write_ptr += 1;
                }
            }

            self.source.rewind();

            for sector in 0..sectors {
                let mut sector_data = [0u8; 256];

                if sector == 0 {
                    sector_data[0] = 18;
                    sector_data[1] = 1;
                    sector_data[2] = 0x41;
                    for b in sector_data.iter_mut().take(144).skip(4) {
                        *b = 0;
                    }
                    sector_data[144..160].copy_from_slice(self.source.name());
                    sector_data[160..162].copy_from_slice(b"\xA0\xA0");
                    sector_data[162..164].copy_from_slice(&self.id);
                    sector_data[164] = 0xA0;
                    sector_data[165] = 0x32;
                    sector_data[166] = 0x41;
                } else if sector == 1 || sector == 2 {
                    if sector == 1 {
                        self.sector1_idx = write_ptr + 39;
                    } else {
                        self.sector2_idx = write_ptr + 39;
                    }

                    if let Some(entry) = self.shortcut_entry {
                        if sector == 1 {
                            self.encode_directory_sector(&[entry], 0, 0xFF, &mut sector_data);
                        } else {
                            // Sector 2 is empty/end
                            self.encode_directory_sector(&[], 0, 0xFF, &mut sector_data);
                        }
                    } else {

                        let mut entries = [crate::storage::DirectoryEntry::new(b"", 0, 0, 0); 8];
                        let mut count = 0;
                        for entry_slot in &mut entries {
                            if let Some(entry) = self.source.next_entry() {
                                *entry_slot = entry;
                                count += 1;
                            } else {
                                break;
                            }
                        }
                        let (next_t, next_s) = if count == 0 {
                            (0, 0xFF)
                        } else {
                            (18, if sector == 1 { 2 } else { 1 })
                        };
                        self.encode_directory_sector(&entries[..count], next_t, next_s, &mut sector_data);
                    }
                }

                for _ in 0..10 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0xFF;
                        write_ptr += 1;
                    }
                }
                let checksum = (sector as u8) ^ track ^ self.id[0] ^ self.id[1];
                let header_raw = [
                    0x08, checksum, sector as u8, track, self.id[1], self.id[0], 0x0F, 0x0F,
                ];
                for chunk in header_raw.chunks_exact(4) {
                    let mut dst = [0u8; 5];
                    gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                    if write_ptr + 5 <= dest.len() {
                        dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                        write_ptr += 5;
                    }
                }
                for _ in 0..9 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0xAA;
                        write_ptr += 1;
                    }
                }
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
                for chunk in data_raw.chunks_exact(4) {
                    let mut dst = [0u8; 5];
                    gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                    if write_ptr + 5 <= dest.len() {
                        dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                        write_ptr += 5;
                    }
                }
                for _ in 0..8 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0xAA;
                        write_ptr += 1;
                    }
                }
            }

            self.source.rewind();
            self.force_update_directory_sectors(dest);
            return Ok(TrackLayout::Accurate(7142));
        }

        // Generate Skeleton Track for other tracks
        let sectors = match track {
            1..=17 => 21,
            18..=24 => 19,
            25..=30 => 18,
            31..=40 => 17,
            _ => 17,
        };

        let mut write_ptr = 0;
        // Lead-in
        for _ in 0..40 {
            if write_ptr < dest.len() {
                dest[write_ptr] = 0xAA;
                write_ptr += 1;
            }
        }

        for sector in 0..sectors {
            // Header SYNC
            for _ in 0..10 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }
            let checksum = (sector as u8) ^ track ^ self.id[0] ^ self.id[1];
            let header_raw = [
                0x08, checksum, sector as u8, track, self.id[1], self.id[0], 0x0F, 0x0F,
            ];
            for chunk in header_raw.chunks_exact(4) {
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }
            // Header-to-Data Gap
            for _ in 0..9 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xAA;
                    write_ptr += 1;
                }
            }
            // Data SYNC
            for _ in 0..10 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }
            // Empty Data Block (0x07 mark + 256 bytes 0 + 0 checksum + 0 offbyte)
            let mut data_raw = [0u8; 260];
            data_raw[0] = 0x07;
            for chunk in data_raw.chunks_exact(4) {
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }
            // Inter-sector gap
            for _ in 0..8 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xAA;
                    write_ptr += 1;
                }
            }
        }

        let track_bytes = match track {
            1..=17 => 7692,
            18..=24 => 7142,
            25..=30 => 6666,
            _ => 6250,
        };

        Ok(TrackLayout::Warp(track_bytes))
    }
}

pub struct D64Image<S: Read + Seek> {
    storage: S,
    id: [u8; 2],
}

impl<S: Read + Seek> D64Image<S> {
    pub fn new(storage: S) -> Self {
        Self {
            storage,
            id: [0x31, 0x31],
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
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error> {
        if !half_track.is_multiple_of(2) {
            return Ok(TrackLayout::Void);
        }
        let track = half_track / 2 + 1;
        if !(1..=40).contains(&track) {
            return Err(Error::InvalidTrack);
        }
        let sectors = Self::get_sectors_per_track(track);
        let track_offset = Self::get_track_offset(track);
        self.storage
            .seek(SeekFrom::Start(track_offset as u64))
            .await
            .map_err(|_| Error::Io)?;
        let mut write_ptr = 0;
        for _ in 0..40 {
            if write_ptr < dest.len() {
                dest[write_ptr] = 0xAA;
                write_ptr += 1;
            }
        }
        for sector in 0..sectors {
            let mut sector_data = [0u8; 256];
            self.storage
                .read_exact(&mut sector_data)
                .await
                .map_err(|_| Error::Io)?;
            for _ in 0..10 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }
            let checksum = sector ^ track ^ self.id[0] ^ self.id[1];
            let header_raw = [
                0x08, checksum, sector, track, self.id[1], self.id[0], 0x0F, 0x0F,
            ];
            for chunk in header_raw.chunks_exact(4) {
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }
            for _ in 0..9 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xAA;
                    write_ptr += 1;
                }
            }
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
            for chunk in data_raw.chunks_exact(4) {
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }
            for _ in 0..8 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xAA;
                    write_ptr += 1;
                }
            }
        }
        let track_bytes = match track {
            1..=17 => 7692,
            18..=24 => 7142,
            25..=30 => 6666,
            _ => 6250,
        };
        Ok(TrackLayout::Accurate(track_bytes))
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
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error> {
        log::info!("Reading G64 track {}", half_track);
        // G64 images typically only have full tracks (even half_track indices).
        // Track 1.0 = HT 0, Track 18.0 = HT 34.
        if half_track > 83 {
            return Ok(TrackLayout::Void);
        }
        let g64_idx = half_track; // (0..83)
        let track_ptr_offset = 12 + (g64_idx as u64) * 4;
        self.storage
            .seek(SeekFrom::Start(track_ptr_offset))
            .await
            .map_err(|_| Error::Io)?;
        let mut ptr_bytes = [0u8; 4];
        self.storage
            .read_exact(&mut ptr_bytes)
            .await
            .map_err(|_| Error::Io)?;
        let track_offset = u32::from_le_bytes(ptr_bytes);
        
        if track_offset == 0 {
            return Ok(TrackLayout::Void);
        }
        self.storage
            .seek(SeekFrom::Start(track_offset as u64))
            .await
            .map_err(|_| Error::Io)?;
        let mut len_bytes = [0u8; 2];
        self.storage
            .read_exact(&mut len_bytes)
            .await
            .map_err(|_| Error::Io)?;
        let track_len = u16::from_le_bytes(len_bytes) as usize;

        if track_len > dest.len() {
            return Err(Error::TrackTooLarge);
        }
        self.storage
            .read_exact(&mut dest[0..track_len])
            .await
            .map_err(|_| Error::Io)?;
        Ok(TrackLayout::Accurate(track_len))
    }
}

pub enum AnyDiskImage<S: Read + Seek, D: DirectorySource> {
    D64(D64Image<S>),
    G64(G64Image<S>),
    VirtualDisk(VirtualDiskImage<S, D>),
}

impl<S: Read + Seek, D: DirectorySource> AnyDiskImage<S, D> {
    pub fn as_virtual_disk_mut(&mut self) -> Option<&mut VirtualDiskImage<S, D>> {
        if let Self::VirtualDisk(img) = self {
            Some(img)
        } else {
            None
        }
    }
}

impl<S: Read + Seek, D: DirectorySource> DiskImage for AnyDiskImage<S, D> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<TrackLayout, Error> {
        match self {
            Self::D64(img) => img.read_track(half_track, dest).await,
            Self::G64(img) => img.read_track(half_track, dest).await,
            Self::VirtualDisk(img) => img.read_track(half_track, dest).await,
        }
    }
}
