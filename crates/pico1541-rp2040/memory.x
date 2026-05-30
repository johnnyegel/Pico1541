MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 256
    FLASH : ORIGIN = 0x10000100, LENGTH = 1536K - 256
    FS    : ORIGIN = 0x10180000, LENGTH = 512K
    RAM   : ORIGIN = 0x20000000, LENGTH = 192K
    SRAM3 : ORIGIN = 0x20030000, LENGTH = 64K
    SCRATCH_A : ORIGIN = 0x20040000, LENGTH = 4K
    SCRATCH_B : ORIGIN = 0x20041000, LENGTH = 4K
}

SECTIONS {
    .boot2 : {
        KEEP(*(.boot2))
    } > BOOT2

    .fs : {
        KEEP(*(.fs))
    } > FS

    .scratch_b (NOLOAD) : ALIGN(4) {
        *(.scratch_b);
        . = ALIGN(4);
    } > SCRATCH_B

    .core1_stack (NOLOAD) : ALIGN(4) {
        *(.core1_stack .core1_stack.*);
        . = ALIGN(4);
    } > SRAM3
}
INSERT AFTER .bss;
