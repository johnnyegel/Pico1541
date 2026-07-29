# Fastload issues

- Seems to be slightly slow
- Proably mis-alignment of track position when changing tracks.
- IEC Reset seems to reset head position, but keeps the track buffer
  When reading the track, the disk sees Half-track 22, but the drive emulator is at track 34. Cause the wrong track to be read,
  and the drive (at least fast-loader) fails with an error.

# TODO

- Refactor code to create a crate dedicated for all Core 1 (emulator code), to make SRAM mapping easy and consistent.
 
