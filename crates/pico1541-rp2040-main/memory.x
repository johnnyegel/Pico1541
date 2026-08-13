MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 256
    FLASH : ORIGIN = 0x10000100, LENGTH = 1536K - 256
    FS    : ORIGIN = 0x10180000, LENGTH = 512K
    RAM   : ORIGIN = 0x20000000, LENGTH = 256K
    SCRATCH_X : ORIGIN = 0x20040000, LENGTH = 4K
    SCRATCH_Y : ORIGIN = 0x20041000, LENGTH = 4K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);
_core1_stack_start = ORIGIN(SCRATCH_Y) + LENGTH(SCRATCH_Y);

SECTIONS {
    .boot2 : {
        KEEP(*(.boot2))
    } > BOOT2

    .fs : {
        KEEP(*(.fs))
    } > FS

    .scratch_x (NOLOAD) : ALIGN(4) {
        *(.scratch_x);
        . = ALIGN(4);
    } > SCRATCH_X

    .scratch_y (NOLOAD) : ALIGN(4) {
        *(.scratch_y);
        . = ALIGN(4);
    } > SCRATCH_Y

    .core1_stack (NOLOAD) : ALIGN(4) {
        *(.core1_stack .core1_stack.*);
        . = ALIGN(4);
    } > SCRATCH_Y


    .ram_rodata : ALIGN(4) {
        __sram_rodata = .;
        *pico1541_emulator*(.rodata .rodata.*);
        *racc6502v4*(.rodata .rodata.*);
        . = ALIGN(4);
        __eram_rodata = .;
    } > RAM AT > FLASH
    __siram_rodata = LOADADDR(.ram_rodata);
}
INSERT AFTER .data;
