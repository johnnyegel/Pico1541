use embassy_rp::pac;
pub fn test() {
    unsafe { pac::TIMER.intf().write(|w| w.set_alarm_3(true)); }
}
