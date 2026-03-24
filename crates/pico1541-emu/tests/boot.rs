use pico1541_emu::Emulator;
use pico1541_emu::activetrack::ActiveTrack;
use pico1541_emu::HardwareBridge;
use pico1541_emu::Command;
use core::sync::atomic::Ordering;

fn init_logger() {
    let _ = env_logger::builder()
        .filter_module("mos6502", log::LevelFilter::Warn)
        .filter_module("pico1541_emu", log::LevelFilter::Info)
        .is_test(true)
        .try_init();
}

fn get_rom() -> [u8; 16384] {
    let rom = include_bytes!("../../../roms/1541-II.251968-03.bin");
    let mut rom_array = [0u8; 16384];
    rom_array.copy_from_slice(rom);
    rom_array
}

#[test]
fn test_drive_boot_behavior() {
    init_logger();
    let rom_array = get_rom();

    // Set up hardware
    static TRACK: ActiveTrack = ActiveTrack::new();
    TRACK.disk_inserted.store(false, Ordering::Relaxed);

    static mut COMMAND_QUEUE: heapless::spsc::Queue<Command, 16> = heapless::spsc::Queue::new();
    let (cmd_tx, mut cmd_rx) = unsafe { (*core::ptr::addr_of_mut!(COMMAND_QUEUE)).split() };
    
    let mut bridge = HardwareBridge::new(&TRACK, cmd_tx);
    // Ensure IEC pins are in a high state (pull-ups active)
    // 1541 VIA1 Port B: bit 7=ATN, bit 4=CLK, bit 1=DATA. All are active-LOW.
    // Setting all to 1 (0xFF) means nothing is asserted.
    bridge.iec_pins = 0xFF;

    let mut drive = Emulator::new(rom_array, bridge);
    drive.reset();

    let mut min_half_track = drive.half_track();
    let mut led_on_count = 0;
    let mut motor_ever_on = false;

    // Run for a very long time (200M instructions)
    for i in 0..200_000_000 {
        drive.step();

        let ht = drive.half_track();
        if ht < min_half_track {
            min_half_track = ht;
        }

        while let Some(cmd) = cmd_rx.dequeue() {
            match cmd {
                Command::SetLed(enabled) => {
                    if enabled { 
                        led_on_count += 1;
                        log::info!("[Test] LED ON at step {}", i);
                    } else {
                        log::info!("[Test] LED OFF at step {}", i);
                    }
                }
                Command::SetMotor(enabled) => {
                    if enabled { motor_ever_on = true; }
                }
                _ => {}
            }
        }

        // Ready state check
        if drive.pc() == 0xE9CD || drive.pc() == 0xEC14 {
             if min_half_track == 0 {
                log::info!("SUCCESS: Reached idle loop and homed head at step {}", i);
                return;
             }
        }

        if i % 20_000_000 == 0 {
            log::info!("Progress: step {}, PC=${:04X}, HT={}", i, drive.pc(), ht);
        }
    }

    log::error!("FAILURE: Boot behavior not matching expectations.");
    log::error!("Final PC=${:04X}, Min HT={}, LED Count={}, Motor Ever ON={}", 
               drive.pc(), min_half_track, led_on_count, motor_ever_on);
    
    panic!("Boot sequence failed to home head");
}
