use embassy_rp::pac;
fn test() {
    let ch = pac::DMA.ch(0);
    ch.trans_count().write_value(10);
    ch.ctrl_trig().write(|w| w.set_en(true));
}
