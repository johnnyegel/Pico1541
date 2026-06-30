use embassy_rp::pac;

pub fn abort_dma(ch: usize) {
    let dma = pac::DMA;
    dma.chan_abort().write(|w| {
        // RP2040 chan_abort takes a bitmask of channels to abort
        // Actually the PAC might have a different method. Let's see what the PAC provides.
    });
}
