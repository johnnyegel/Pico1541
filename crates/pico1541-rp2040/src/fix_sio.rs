use embassy_rp::pac;

pub fn read_gpio1() -> bool {
    let sio = pac::SIO;
    sio.gpio_in().read().gpio(1)
}
