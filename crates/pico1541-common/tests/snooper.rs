use pico1541_common::snooper::IecSnoopByteHandler;
use pico1541_common::interface::DriveEvent;

#[test]
fn test_handler_detects_cd_command() {
    let mut handler = IecSnoopByteHandler::new();
    
    // Simulate events from snooper
    handler.handle(DriveEvent::IecSnoopSecondaryAddress(15));
    handler.handle(DriveEvent::IecSnoopByteReceived(b'C', false));
    handler.handle(DriveEvent::IecSnoopByteReceived(b'D', false));
    handler.handle(DriveEvent::IecSnoopByteReceived(b':', false));
    handler.handle(DriveEvent::IecSnoopByteReceived(b'T', false));
    handler.handle(DriveEvent::IecSnoopByteReceived(b'E', false));
    handler.handle(DriveEvent::IecSnoopByteReceived(b'S', false));
    handler.handle(DriveEvent::IecSnoopByteReceived(b'T', true));
    
    assert!(handler.is_hijacked());
    assert!(handler.should_discard());
    assert_eq!(handler.get_command(), Some("CD:TEST"));
}

#[test]
fn test_handler_ignores_other_commands() {
    let mut handler = IecSnoopByteHandler::new();
    
    handler.handle(DriveEvent::IecSnoopSecondaryAddress(15));
    handler.handle(DriveEvent::IecSnoopByteReceived(b'I', false));
    handler.handle(DriveEvent::IecSnoopByteReceived(b'0', true));
    
    assert!(!handler.is_hijacked());
    assert!(!handler.should_discard());
    assert_eq!(handler.get_command(), Some("I0"));
}

#[test]
fn test_handler_resets_on_new_addressing() {
    let mut handler = IecSnoopByteHandler::new();
    
    handler.handle(DriveEvent::IecSnoopByteReceived(b'C', false));
    handler.handle(DriveEvent::IecSnoopByteReceived(b'D', false));
    handler.handle(DriveEvent::IecSnoopByteReceived(b':', false));
    assert!(handler.is_hijacked());
    
    handler.handle(DriveEvent::IecSnoopSecondaryAddress(0));
    assert!(!handler.is_hijacked());
    assert_eq!(handler.get_command(), None);
}
