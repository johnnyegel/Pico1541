MEMORY {
    FLASH : ORIGIN = 0x10000000, LENGTH = 1536K
    FS    : ORIGIN = 0x10180000, LENGTH = 2048K
    RAM   : ORIGIN = 0x20000000, LENGTH = 256K
    CORE1_RAM : ORIGIN = 0x20040000, LENGTH = 256K
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

    .core1_uninit (NOLOAD) : ALIGN(4) {
        *(.core1_uninit .core1_uninit.*);
        . = ALIGN(4);
    } > CORE1_RAM

    .core1_fast_bss (NOLOAD) : ALIGN(4) {
        *(.core1_fast_bss .core1_fast_bss.*);
        . = ALIGN(4);
    } > SCRATCH_X

    .core1_stack (NOLOAD) : ALIGN(4) {
        *(.core1_stack .core1_stack.*);
        . = ALIGN(4);
    } > SCRATCH_Y

    .core1_text : ALIGN(4) {
        __score1_text = .;
        *(.core1_text .core1_text.*);
        . = ALIGN(4);
        __ecore1_text = .;
    } > CORE1_RAM AT > FLASH
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
    .shared_uninit (NOLOAD) : ALIGN(8192) {
        *(.shared_uninit .shared_uninit.*);
    } > RAM
}
INSERT AFTER .uninit;
