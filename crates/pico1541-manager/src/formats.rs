use crate::gcr;
use embedded_io_async::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub enum Error {
    Io,
    InvalidFormat,
    InvalidTrack,
    TrackTooLarge,
}

pub trait DiskImage {
    /// Loads a specific half-track into the provided buffer.
    /// Returns the number of bits in the track.
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, Error>;
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
            // Half-track (e.g. 1.5, 2.5) are usually empty in D64
            dest.fill(0x55); // Filler
            return Ok(35000); // Nominal bit length
        }

        let track = half_track / 2;
        if track < 1 || track > 40 {
            return Err(Error::InvalidTrack);
        }

        let sectors = Self::get_sectors_per_track(track);
        let track_offset = Self::get_track_offset(track);

        self.storage.seek(SeekFrom::Start(track_offset as u64)).await.map_err(|_| Error::Io)?;

        let mut write_ptr = 0;
        
        for sector in 0..sectors {
            let mut sector_data = [0u8; 256];
            self.storage.read_exact(&mut sector_data).await.map_err(|_| Error::Io)?;

            // 1. Header Sync
            for _ in 0..8 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }

            // 2. Header Block (GCR encoded)
            // Header: $08, checksum, sector, track, id2, id1, $0F, $0F
            let checksum = sector ^ track ^ self.id[0] ^ self.id[1];
            let header_raw = [0x08, checksum, sector, track, self.id[1], self.id[0], 0x0F, 0x0F];
            
            // Encode header (8 bytes -> 10 bytes GCR)
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

            // 3. Header Gap
            for _ in 0..8 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0x55;
                    write_ptr += 1;
                }
            }

            // 4. Data Sync
            for _ in 0..8 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }

            // 5. Data Block (GCR encoded)
            // Data Header: $07, 256 bytes data, checksum, $00, $00
            let mut data_checksum = 0u8;
            for b in sector_data.iter() {
                data_checksum ^= b;
            }
            
            // Data block is 260 bytes: $07, 256 data, checksum, 0, 0
            // 260 bytes -> 260 / 4 * 5 = 65 blocks * 5 = 325 bytes GCR
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

            // 6. Inter-sector Gap
            for _ in 0..8 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0x55;
                    write_ptr += 1;
                }
            }
        }

        // Fill remaining track with gap bytes
        let nominal_bytes = match track {
            1..=17 => 6250,
            18..=24 => 5625,
            25..=30 => 5000,
            _ => 4375,
        };

        while write_ptr < nominal_bytes && write_ptr < dest.len() {
            dest[write_ptr] = 0x55;
            write_ptr += 1;
        }

        Ok((write_ptr as u32) * 8)
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

        // G64 Header: "GCR-1541", Version, Tracks, Max Track Length
        // Track offsets start at byte 12. 84 tracks * 4 bytes = 336 bytes.
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
