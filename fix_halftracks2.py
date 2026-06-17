import re

with open("crates/pico1541-common/src/formats.rs", "r") as f:
    content = f.read()

# Restore VirtualDiskImage
content = content.replace(
    "if half_track > 85 {\n            return Ok((TrackLayout::Void, 0));\n        }",
    "if half_track > 85 || (half_track % 2) != 0 {\n            return Ok((TrackLayout::Void, 0));\n        }"
)

# Restore D64Image
content = re.sub(
    r"for half_track in 0\.\.=85 \{\s*let track = half_track / 2 \+ 1;\s*if track <= 40 \{\s*lookup_table\.offsets\[half_track as usize\] = Some\(Self::get_track_offset\(track\)\);\s*\}\s*\}",
    """for half_track in 0..=84 {
            if half_track % 2 == 0 {
                let track = half_track / 2 + 1;
                lookup_table.offsets[half_track as usize] = Some(Self::get_track_offset(track));
            }
        }""",
    content
)

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.write(content)
