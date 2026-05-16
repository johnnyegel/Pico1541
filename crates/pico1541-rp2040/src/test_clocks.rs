use embassy_rp::config::Config;
use embassy_rp::clocks::Config as ClocksConfig;

fn main() {
    let mut config = Config::default();
    // Try to find a way to set 250MHz
    // config.clocks = ClocksConfig::crystal_250mhz(); 
}
