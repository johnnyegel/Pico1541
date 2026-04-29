use pico1541_common::iec::{IecBus, IecBusCom};
use core::cell::RefCell;
use pollster::block_on;

struct MockBus {
    atn: RefCell<bool>,
    clk: RefCell<bool>,
    data: RefCell<bool>,
    drive_clk: RefCell<bool>,
    drive_data: RefCell<bool>,
}

impl MockBus {
    fn new() -> Self {
        Self {
            atn: RefCell::new(false),
            clk: RefCell::new(false),
            data: RefCell::new(false),
            drive_clk: RefCell::new(false),
            drive_data: RefCell::new(false),
        }
    }
}

impl IecBus for &MockBus {
    fn set_atn(&mut self, assert: bool) { *self.atn.borrow_mut() = assert; }
    fn set_clk(&mut self, assert: bool) { *self.clk.borrow_mut() = assert; }
    fn set_data(&mut self, assert: bool) { *self.data.borrow_mut() = assert; }
    fn get_clk(&self) -> bool { *self.clk.borrow() || *self.drive_clk.borrow() }
    fn get_data(&self) -> bool { *self.data.borrow() || *self.drive_data.borrow() }
    async fn delay_us(&self, _us: u32) {}
}

#[test]
fn test_iec_atn_handshake() {
    let bus = MockBus::new();
    
    // Correct protocol: Drive acknowledges ATN by pulling DATA low
    *bus.drive_data.borrow_mut() = true;

    let com = IecBusCom::new(&bus);
    let com = block_on(com.atn_assert()).unwrap();
    
    assert!(*bus.atn.borrow(), "ATN should be asserted");
    assert!(*bus.clk.borrow(), "Host should pull CLK low after ATN ack");
    assert!(!*bus.data.borrow(), "Host should release DATA after ATN ack");
    
    let _ = com.done();
}

#[test]
fn test_iec_send_byte() {
    let bus = MockBus::new();
    
    // Reactive mock to handle listener side of send
    struct SendMock<'a>(&'a MockBus, RefCell<usize>);
    impl IecBus for SendMock<'_> {
        fn set_atn(&mut self, a: bool) { self.0.set_atn(a); }
        fn set_clk(&mut self, a: bool) { 
            self.0.set_clk(a);
            if a { *self.1.borrow_mut() += 1; }
        }
        fn set_data(&mut self, a: bool) { self.0.set_data(a); }
        fn get_clk(&self) -> bool { self.0.get_clk() }
        fn get_data(&self) -> bool { 
            let count = *self.1.borrow();
            if count == 0 {
                false // Ready to receive (DATA released)
            } else if count >= 8 {
                true // Acknowledge (DATA pulled low)
            } else {
                false
            }
        }
        async fn delay_us(&self, _us: u32) {}
    }

    let mock = SendMock(&bus, RefCell::new(0));
    let com = IecBusCom::new(mock);
    let com = block_on(com.send(0x55)).unwrap();
    
    let _ = com.done();
}

#[test]
fn test_iec_recv_byte() {
    let bus = MockBus::new();
    
    // Reactive mock to handle talker side of recv
    struct RecvMock<'a>(&'a MockBus, RefCell<usize>);
    impl IecBus for RecvMock<'_> {
        fn set_atn(&mut self, a: bool) { self.0.set_atn(a); }
        fn set_clk(&mut self, a: bool) { self.0.set_clk(a); }
        fn set_data(&mut self, a: bool) { self.0.set_data(a); }
        fn get_clk(&self) -> bool { 
            let count = *self.1.borrow();
            if count > 0 && count <= 16 {
                count % 2 == 1 // Toggle CLK
            } else {
                false
            }
        }
        fn get_data(&self) -> bool { 
            // Return 0xAA (10101010)
            let count = *self.1.borrow();
            if count == 0 { return false; }
            let bit_idx = (count - 1) / 2;
            if bit_idx < 8 {
                (0xAA >> bit_idx) & 1 == 1
            } else {
                false
            }
        }
        async fn delay_us(&self, _us: u32) {
             *self.1.borrow_mut() += 1;
        }
    }

    let mock = RecvMock(&bus, RefCell::new(0));
    let com = IecBusCom::new(mock);
    let mut byte = 0;
    let com = block_on(com.recv(&mut byte)).unwrap();
    
    assert_eq!(byte, 0xAA);
    let _ = com.done();
}
