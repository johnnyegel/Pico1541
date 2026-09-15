use pico1541_common::interface::DriveEvent;
use pico1541_common::snooper::IecSnoopByteHandler;

#[test]
fn test_handler_detects_cd_command() {
    let mut handler = IecSnoopByteHandler::new();

    // Simulate events from snooper
    handler.handle(DriveEvent::IecSnoopSecondaryAddress(15), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b'C', false), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b'D', false), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b':', false), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b'T', false), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b'E', false), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b'S', false), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b'T', true), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopEndOfTransaction, &mut |_| {});
    handler.handle(DriveEvent::IecSnoopTransactionComplete, &mut |_| {});

    assert!(handler.is_hijacked());
    assert!(handler.should_discard());
    assert_eq!(handler.get_command(), Some("CD:TEST"));
}

#[test]
fn test_handler_ignores_other_commands() {
    let mut handler = IecSnoopByteHandler::new();

    handler.handle(DriveEvent::IecSnoopSecondaryAddress(15), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b'I', false), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b'0', true), &mut |_| {});

    assert!(!handler.is_hijacked());
    assert!(!handler.should_discard());
    assert_eq!(handler.get_command(), Some("I0"));
}

#[test]
fn test_handler_resets_on_new_addressing() {
    let mut handler = IecSnoopByteHandler::new();

    handler.handle(DriveEvent::IecSnoopByteReceived(b'C', false), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b'D', false), &mut |_| {});
    handler.handle(DriveEvent::IecSnoopByteReceived(b':', false), &mut |_| {});
    assert!(handler.is_hijacked());

    handler.handle(DriveEvent::IecSnoopSecondaryAddress(0), &mut |_| {});
    assert!(!handler.is_hijacked());
    assert_eq!(handler.get_command(), None);
}
