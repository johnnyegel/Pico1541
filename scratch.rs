use embassy_rp::pac;
fn test() {
    let dma = pac::DMA;
    let ch_a = dma.ch(0);
    let _ = ch_a.al3_read_addr_trig();
}
