import re

with open("crates/pico1541-common/src/formats.rs", "r") as f:
    content = f.read()

# Fix VirtualDiskImage
content = content.replace(
    "if half_track > 85 || (half_track % 2) != 0 {\n            return Ok((TrackLayout::Void, 0));\n        }",
    "if half_track > 85 {\n            return Ok((TrackLayout::Void, 0));\n        }"
)

# Fix D64Image
content = re.sub(
    r"for half_track in 0\.\.=84 \{\s*if half_track % 2 == 0 \{\s*let track = half_track / 2 \+ 1;\s*lookup_table\.offsets\[half_track as usize\] = Some\(Self::get_track_offset\(track\)\);\s*\}\s*\}",
    """for half_track in 0..=85 {
            let track = half_track / 2 + 1;
            if track <= 40 {
                lookup_table.offsets[half_track as usize] = Some(Self::get_track_offset(track));
            }
        }""",
    content
)

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.write(content)
