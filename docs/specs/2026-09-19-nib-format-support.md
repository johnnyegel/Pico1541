# NIB Disk Image Format Support & NBZ Decompression Tool

**Date:** 2026-09-19  
**Status:** Approved  
**Target:** `pico1541-manager`, `tools/nbz2nib`

---

## 1. Overview & Rationale

Preservation archives for the Commodore 64 frequently store original disks in **.NIB** (MNIB / nibtools raw nibble dump) or **.NBZ** (LZ77-compressed NIB) formats. While standard **.D64** images store high-level sector data, they lose copy-protection structures, non-standard sync marks, and custom bitrates.

The **.NIB** format provides a raw, bit-level capture of each track at a fixed length of 8,192 bytes (`0x2000`), storing GCR data directly from the drive head.

Due to the RP2040's limited SRAM (264 KB) and the whole-file nature of NBZ's LZ77 stream (~672 KB uncompressed), real-time on-the-fly decompression during emulation is unfeasible. Instead, this specification defines a two-part solution:
1. **Firmware:** Direct, zero-extra-RAM, cycle-exact support for uncompressed **.NIB** files in `pico1541-manager`.
2. **Host Tooling:** A dedicated, standalone Rust CLI tool (`nbz2nib`) in the workspace to batch decompress `.nbz` files into `.nib` files for the SD card.

---

## 2. Technical Format Specifications

### 2.1 The NIB Format
A `.NIB` file has a fixed 256-byte (`0x100`) header followed by sequential, raw 8,192-byte track blocks:

* **Header (Bytes `0x00..0xFF`):**
  * `0x00..0x0C` (13 bytes): Magic ASCII identifier `"MNIB-1541-RAW"` or `"MNIB-1571-RAW"`.
  * `0x0D` (1 byte): Version number (e.g. `0x01`, `0x02`, or `0x03`).
  * `0x0E..0x0F` (2 bytes): Reserved / flags.
  * `0x10..0xFF`: Track mapping table containing sequential pairs of `(track_num, density)`:
    * `track_num` (1 byte): Half-track number encoded as $2 \times \text{track}$. Specifically:
      * Track 1.0 = `2`
      * Track 1.5 = `3`
      * Track 18.0 = `36`
      * The list terminates when `track_num == 0` or after a maximum of 84 entries.
    * `density` (1 byte): Density / speed zone flag:
      * Bits 0..1: Speed zone (`0..3`).
      * Bits 2..7: Custom flags (`BM_NO_SYNC`, `BM_FF_TRACK`, etc.).

* **Track Data (Offset `0x100` onwards):**
  * Each track entry in the header corresponds sequentially to an 8,192-byte (`0x2000`) block in the file:
    $$\text{Offset} = 0x100 + (\text{entry\_index} \times 8192)$$
  * Track data consists of raw 8-bit GCR-encoded bytes as read from the drive head.

* **Track Mapping to Pico1541:**
  * Pico1541 internal half-tracks are 0-indexed (Track 1.0 = `0`, Track 18.0 = `34`):
    $$\text{pico\_half\_track} = \text{track\_num} - 2$$
  * Fits natively into Pico1541's `TRACK_SIZE: usize = 8192`.

### 2.2 The NBZ Format
An `.NBZ` file is an LZ77-compressed `.NIB` stream utilizing Marcus Geelnard's algorithm:
* Byte 0: `marker` (the least common byte in the input).
* Subsequent stream:
  * Literal bytes (when `symbol != marker`).
  * Marker escape: `marker followed by 0x00` represents a literal marker byte.
  * History reference: `marker followed by non-zero byte`:
    * Length: decoded via variable-length 7-bit integer (`_LZ_ReadVarSize`).
    * Offset: decoded via variable-length 7-bit integer (`_LZ_ReadVarSize`).
    * Copy `length` bytes from `current_pos - offset` in the output history buffer.

---

## 3. Architecture & Implementation

### 3.1 Host Tool: `tools/nbz2nib`
A new workspace binary crate located at `tools/nbz2nib` (or `crates/nbz2nib`):

* **CLI Usage:**
  ```bash
  # Convert a single file
  cargo run -p nbz2nib -- input.nbz
  cargo run -p nbz2nib -- input.nbz -o custom.nib

  # Batch convert a directory
  cargo run -p nbz2nib -- /path/to/nbz/folder/
  ```
* **Core Modules:**
  * `lz77.rs`: Clean, safe Rust implementation of the Marcus Geelnard LZ77 decompressor.
  * `main.rs`: CLI argument parsing, input/output file management, header validation.
* **Validation:**
  * Verifies decompressed bytes start with `"MNIB-1541-RAW"` or `"MNIB-1571-RAW"`.
  * Reports track count and decompressed size.
* **Testing:**
  * Unit tests validating round-trip decompressions and edge cases.

---

### 3.2 Firmware: `NibImage` in `pico1541-manager`

#### Format Driver (`crates/pico1541-manager/src/formats/nib.rs`)
Implements `DiskImage` for `NibImage<S: Read + Seek>`:

```rust
pub struct NibImage<S: Read + Seek> {
    storage: S,
    lookup_table: TrackLookupTable,
}
```

1. **`from_storage(mut storage: S) -> Result<Self, Error>`:**
   - Reads 256 bytes from offset 0.
   - Validates header magic: `&header[0..13] == b"MNIB-1541-RAW"` or `b"MNIB-1571-RAW"`.
   - Iterates through the track table starting at `0x10` (up to 84 pairs):
     - `track_num = header[0x10 + entry * 2]`
     - `density = header[0x10 + entry * 2 + 1]`
     - If `track_num == 0`: break.
     - If `track_num >= 2`:
       - `half_track = (track_num - 2) as usize`
       - If `half_track < 84`:
         - `lookup_table.offsets[half_track] = Some(0x100 + (entry as u32 * 8192))`
         - `lookup_table.speed_zones[half_track] = Some(density & 0x03)`
   - Returns `Ok(Self { storage, lookup_table })`.

2. **`DiskImage::read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<(TrackLayout, usize, Option<u8>), Error>`:**
   - Looks up `half_track`:
     - If `None`: returns `Ok((TrackLayout::Void, 0, None))`.
     - If `Some(offset)`:
       - Seeks to `offset`.
       - Reads 8,192 bytes directly into `dest[..8192]`.
       - Returns `Ok((TrackLayout::Accurate, 8192, speed_zone))`.

---

### 3.3 Manager & UI Integration

1. **`crates/pico1541-manager/src/lib.rs`:**
   - Update `MountPayload`:
     ```rust
     pub enum MountPayload {
         VirtualDirectory(heapless::String<128>, bool),
         D64(heapless::String<128>, bool),
         G64(heapless::String<128>, bool),
         Nib(heapless::String<128>, bool),
     }
     ```

2. **`crates/pico1541-manager/src/formats/mod.rs`:**
   - Expose `pub mod nib; pub use nib::NibImage;`.
   - Update `AnyDiskImage`:
     ```rust
     pub enum AnyDiskImage<S: embedded_io_async::Read + embedded_io_async::Seek + 'static> {
         D64(D64Image<S>),
         G64(G64Image<S>),
         Nib(NibImage<S>),
         VirtualDisk(VirtualDiskImage<PagedDirectorySource<144>>),
     }
     ```

3. **`crates/pico1541-manager/src/tasks/ui/browser.rs`:**
   - Add `.nib` / `.NIB` extension detection:
     ```rust
     let is_nib = filename.ends_with(".nib") || filename.ends_with(".NIB");
     if is_nib {
         #[cfg(feature = "defmt")]
         defmt::info!("Sending Mount for NIB...");
         let _ = ctx.context.cmd_tx.send(crate::InternalCommand::Mount(
             crate::MountPayload::Nib(full_path.clone(), manager.use_internal)
         )).await;
         *ctx.mounted_file_path = Some(full_path);
     }
     ```

4. **`crates/pico1541-manager/src/tasks/mount_manager/mod.rs`:**
   - In `mount_sequence`:
     - Recognize `is_nib`:
       ```rust
       let is_nib = p_str.ends_with(".nib") || p_str.ends_with(".NIB");
       if is_nib {
           let file = if use_internal {
               self.opener.open_internal_file(p_str)
           } else {
               self.opener.open_sd_file(p_str)
           };
           if let Ok(nib) = crate::formats::NibImage::from_storage(file).await {
               self.disk_image = Some(crate::formats::AnyDiskImage::Nib(nib));
           } else {
               self.disk_image = None;
           }
       }
       ```
     - Initial track load delegates directly to `self.handle_track_event(half_track).await`, ensuring unified loading, timing metrics, and buffer swapping.

---

## 4. Verification & Testing Strategy

1. **Host Tool Unit Tests (`tools/nbz2nib`):**
   - Decompress synthetic LZ77 streams with repeating patterns, marker escapes, and multi-byte offsets.
   - Test CLI batch processing and error handling on invalid files.

2. **Manager Unit Tests (`crates/pico1541-manager/tests/formats.rs`):**
   - Create mock memory storage containing a valid `.NIB` header and track payload.
   - Verify `NibImage::from_storage` parses half-tracks, speeds, and offsets correctly.
   - Verify `read_track` returns `TrackLayout::Accurate` and exact 8,192-byte track content.

3. **Hardware Compilation:**
   - Build `pico1541-rp2040` in `--release` mode for `thumbv6m-none-eabi`.
   - Ensure zero compiler warnings and verify binary size impact is negligible (< 1 KB).
