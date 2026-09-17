MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 256
    FLASH : ORIGIN = 0x10000100, LENGTH = 1536K - 256
    FS    : ORIGIN = 0x10180000, LENGTH = 512K
    RAM   : ORIGIN = 0x21000000, LENGTH = 128K
    CORE1_RAM : ORIGIN = 0x21020000, LENGTH = 128K
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
