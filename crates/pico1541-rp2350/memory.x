MEMORY {
    FLASH : ORIGIN = 0x10000000, LENGTH = 1536K
    FS    : ORIGIN = 0x10180000, LENGTH = 512K
    RAM   : ORIGIN = 0x20000000, LENGTH = 256K
    SRAM2 : ORIGIN = 0x20040000, LENGTH = 64K
    SRAM3 : ORIGIN = 0x20050000, LENGTH = 64K
    SCRATCH_X : ORIGIN = 0x20080000, LENGTH = 4K
    SCRATCH_Y : ORIGIN = 0x20081000, LENGTH = 4K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);
_core1_stack_start = ORIGIN(SCRATCH_Y) + LENGTH(SCRATCH_Y);

SECTIONS {
    .start_block : ALIGN(4)
    {
        __start_block_addr = .;
        KEEP(*(.start_block));
        KEEP(*(.boot_info));
    } > FLASH
} INSERT AFTER .vector_table;

_stext = ADDR(.start_block) + SIZEOF(.start_block);

SECTIONS {
    .fs : {
        KEEP(*(.fs))
    } > FS

    .sram3 (NOLOAD) : ALIGN(4) {
        *(.sram3 .sram3.*);
        *(.dual_core_buffer);
        . = ALIGN(4);
    } > SRAM3

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

    .core1_text : ALIGN(4) {
        __score1_text = .;
        *(.core1_text .core1_text.*);
        . = ALIGN(4);
        __ecore1_text = .;
    } > SRAM2 AT > FLASH
    __sicore1_text = LOADADDR(.core1_text);


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

SECTIONS {
    .dual_core_data (NOLOAD) : ALIGN(8192) {
        *(.dual_core_data .dual_core_data.*);
    } > RAM
}
INSERT AFTER .uninit;
