/// Events issues by the drive emulator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveEvent {
    /// The Snooper has detected a secondary address byte addressed to this device
    IecSnoopSecondaryAddress(u8), // secondary address byte
    /// Byte received to this device, on the previous secondary address
    IecSnoopByteReceived(u8, bool), // byte, is_eoi
    /// A complete decoded IEC command (e.g. "CD:...")
    IecCommand {
        data: [u8; 32],
        len: u8,
    },
    /// The Snooper detected a CLOSE command for a specific secondary address
    IecCloseChannel(u8),
    /// The Snooper has detected the end of a transaction (e.g. UNLISTEN or ATN assertion)
    IecSnoopEndOfTransaction,
    IecSnoopTransactionComplete,
    /// The Snooper detected that the IEC bus has been fully idle (all lines released) for > 250ms.
    IecBusIdleTimeout,
}

pub trait HostInterface: crate::context::ViaHardware {
    fn send_event(&mut self, event: DriveEvent);

    /// Returns true if the bus is currently hijacked by the manager.
    fn is_hijacked(&self) -> bool {
        false
    }

    /// Returns true if the manager wants to discard the ROM command buffer.
    /// This should clear the request after returning true.
    fn pop_discard_request(&mut self) -> bool {
        false
    }

    /// Wait until the hardware physical timer aligns with the absolute emulated cycle.
    fn spin_until(&mut self, _target_cycle: u32) -> bool {
        true
    }

    /// Update the IEC addresses (LISTEN and TALK) snooped by the manager.
    /// Usually called by the emulator when it detects a change in drive RAM.
    fn update_iec_addresses(&mut self, _listen: u8, _talk: u8) {}

    fn get_device_id(&self) -> u8 {
        8
    }

    /// Returns the current PC of the CPU (for debugging)
    fn get_pc(&self) -> u16 {
        0
    }

    /// Called when the read/write head moves to a new half-track.
    fn track_changed(&mut self, _half_track: u8) {}

    /// Returns true if the track has changed since the last time this was called.
    fn take_track_changed_flag(&mut self) -> bool {
        false
    }

    /// Called when an SD2IEC "CD" command is intercepted from channel 15
    fn intercept_cd_command(&mut self, _path: &str) {}

    // Execution control
    fn check_halt_requested(&self) -> bool {
        false
    }
    fn report_performance_counters(&mut self, _ticks: u32, _idles: u32) {}
}

/// Decouples cross-core track change notifications via a hardware doorbell.
pub trait TrackHandoff {
    /// Notify the manager that the head has moved to a new half-track.
    fn notify_track_change(&mut self, half_track: u8);
    /// Get the current half-track.
    fn current_track(&self) -> u8;
    /// Wait until a new track is requested.
    #[allow(async_fn_in_trait)] // We only use this internally and do not need Send bounds
    async fn wait_for_change(&mut self) -> u8;
}
