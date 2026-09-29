use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use heapless::Vec;
use pico1541_manager::driver::DisplayDriver;
use pico1541_manager::tasks::menu::{Menu, MenuEvent};
use pico1541_manager::{UiAction, LOCATION_CHANGED};

#[no_mangle]
fn _embassy_time_now() -> u64 {
    0
}

#[no_mangle]
fn _embassy_time_schedule_wake(_at: u64, _waker: &core::task::Waker) {}

struct MockDisplay;

impl DisplayDriver for MockDisplay {
    async fn clear(&mut self) {}
    async fn print(&mut self, _line: u8, _text: &str) {}
    async fn print_large(&mut self, _line: u8, _text: &str) {}
    async fn update(&mut self) {}
    async fn redraw(&mut self) {}
    fn request_redraw(&mut self) {}
    fn is_dirty(&self) -> bool {
        false
    }
}

#[test]
fn test_menu_wakes_on_location_changed() {
    let mut items = Vec::<&'static str, 4>::new();
    let _ = items.push("Item 1");
    let _ = items.push("Item 2");
    let mut menu = Menu::new("Test Menu", items);

    let channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, UiAction, 16>::new()));
    let mut display = MockDisplay;

    // Signal location changed before run
    LOCATION_CHANGED.signal(());

    let event = block_on(menu.run(&mut display, &channel.receiver()));
    match event {
        MenuEvent::LocationChanged => {}
        _ => panic!("Expected MenuEvent::LocationChanged, got other event"),
    }
}

#[test]
fn test_menu_receives_ui_action() {
    let mut items = Vec::<&'static str, 4>::new();
    let _ = items.push("Item 1");
    let _ = items.push("Item 2");
    let mut menu = Menu::new("Test Menu", items);

    let channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, UiAction, 16>::new()));
    let mut display = MockDisplay;

    // Send select action
    let _ = channel.sender().try_send(UiAction::Select);

    let event = block_on(menu.run(&mut display, &channel.receiver()));
    match event {
        MenuEvent::Selected(idx) => assert_eq!(idx, 0),
        _ => panic!("Expected MenuEvent::Selected(0)"),
    }
}
