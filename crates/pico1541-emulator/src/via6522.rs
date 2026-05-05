//! A functional implementation of the MOS 6522 VIA.
//!
//! This module provides a detailed simulation of the 6522 VIA,
//! including timers, interrupts, and I/O ports.

#[cfg(feature = "debug")]
use racc6502::bus::MemoryMonitor;

#[derive(Clone, Copy, Default)]
pub struct Via6522 {
    /// Output Register B
    pub orb: u8,
    /// Output Register A
    pub ora: u8,
    /// Data Direction Register B
    pub ddrb: u8,
    /// Data Direction Register A
    pub ddra: u8,

    /// Timer 1 Counter
    pub t1c: u16,
    /// Timer 1 Latch
    pub t1l: u16,
    /// Timer 1 is running
    pub t1_active: bool,
    /// Timer 1 has timed out (for PB7 square wave)
    pub t1_pb7: bool,

    /// Timer 2 Counter
    pub t2c: u16,
    /// Timer 2 Latch (low byte for writes)
    pub t2l: u8,
    /// Timer 2 is running
    pub t2_active: bool,

    /// Shift Register
    pub sr: u8,
    /// Auxiliary Control Register
    pub acr: u8,
    /// Peripheral Control Register
    pub pcr: u8,
    /// Interrupt Flag Register
    pub ifr: u8,
    /// Interrupt Enable Register
    pub ier: u8,

    /// Input Port A
    pub in_a: u8,
    /// Input Port B
    pub in_b: u8,

    /// IRQ output state
    pub irq: bool,

    // Latched values for Port A/B (ACR control)
    latch_a: u8,
    latch_b: u8,

    // Internal state for edge detection
    last_ca1: bool,
    last_cb1: bool,
    last_ca2: bool,
    last_cb2: bool,

    // Shift register state
    sr_count: u8,
    sr_active: bool,
}

impl Via6522 {
    pub fn new() -> Self {
        Self {
            ier: 0x00,
            in_a: 0xFF, // Default to floating high
            in_b: 0xFF, // Default to floating high
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Read a byte from a VIA register.
    pub fn read(&mut self, addr: u16) -> u8 {
        let register = (addr & 0x0F) as usize;
        match register {
            0x00 => {
                // IRB
                self.ifr &= !0x18; // Clear CB1 and CB2 interrupt flags
                self.update_irq();
                if (self.acr & 0x02) != 0 {
                    self.latch_b
                } else {
                    (self.orb & self.ddrb) | (self.in_b & !self.ddrb)
                }
            }
            0x01 => {
                // IRA
                self.ifr &= !0x03; // Clear CA1 and CA2 interrupt flags
                self.update_irq();
                if (self.acr & 0x01) != 0 {
                    self.latch_a
                } else {
                    (self.ora & self.ddra) | (self.in_a & !self.ddra)
                }
            }
            0x02 => self.ddrb,
            0x03 => self.ddra,
            0x04 => {
                // T1C-L
                self.ifr &= !0x40; // Clear T1 interrupt flag
                self.update_irq();
                (self.t1c & 0xFF) as u8
            }
            0x05 => (self.t1c >> 8) as u8,   // T1C-H
            0x06 => (self.t1l & 0xFF) as u8, // T1L-L
            0x07 => (self.t1l >> 8) as u8,   // T1L-H
            0x08 => {
                // T2C-L
                self.ifr &= !0x20; // Clear T2 interrupt flag
                self.update_irq();
                (self.t2c & 0xFF) as u8
            }
            0x09 => (self.t2c >> 8) as u8, // T2C-H
            0x0A => {
                // SR
                self.ifr &= !0x04; // Clear SR interrupt flag
                self.update_irq();
                self.sr
            }
            0x0B => self.acr,
            0x0C => self.pcr,
            0x0D => self.ifr,
            0x0E => self.ier | 0x80, // Bit 7 is always 1 on read
            0x0F => {
                // IRA (no handshake)
                if (self.acr & 0x01) != 0 {
                    self.latch_a
                } else {
                    (self.ora & self.ddra) | (self.in_a & !self.ddra)
                }
            }
            _ => 0,
        }
    }

    /// Write a byte to a VIA register.
    pub fn write(&mut self, addr: u16, value: u8) {
        let register = (addr & 0x0F) as usize;
        match register {
            0x00 => {
                // ORB
                self.orb = value;
                self.ifr &= !0x18; // Clear CB1 and CB2 interrupt flags
                self.update_irq();
            }
            0x01 => {
                // ORA
                self.ora = value;
                self.ifr &= !0x03; // Clear CA1 and CA2 interrupt flags
                self.update_irq();
            }
            0x02 => self.ddrb = value,
            0x03 => self.ddra = value,
            0x04 => self.t1l = (self.t1l & 0xFF00) | (value as u16), // T1L-L
            0x05 => {
                // T1C-H (starts T1)
                self.t1l = (self.t1l & 0x00FF) | ((value as u16) << 8);
                self.t1c = self.t1l;
                self.t1_active = true;
                self.ifr &= !0x40; // Clear T1 interrupt flag
                self.update_irq();
            }
            0x06 => self.t1l = (self.t1l & 0xFF00) | (value as u16), // T1L-L (latch only)
            0x07 => {
                // T1L-H (latch only)
                self.t1l = (self.t1l & 0x00FF) | ((value as u16) << 8);
                self.ifr &= !0x40; // Clear T1 interrupt flag
                self.update_irq();
            }
            0x08 => self.t2l = value, // T2C-L
            0x09 => {
                // T2C-H (starts T2)
                self.t2c = ((value as u16) << 8) | (self.t2l as u16);
                self.t2_active = true;
                self.ifr &= !0x20; // Clear T2 interrupt flag
                self.update_irq();
            }
            0x0A => {
                // SR
                self.sr = value;
                self.ifr &= !0x04; // Clear SR interrupt flag
                self.update_irq();
                if (self.acr & 0x1C) != 0 {
                    self.sr_active = true;
                    self.sr_count = 8;
                }
            }
            0x0B => self.acr = value,
            0x0C => self.pcr = value,
            0x0D => {
                // IFR
                self.ifr &= !value;
                self.update_irq();
            }
            0x0E => {
                // IER
                if (value & 0x80) != 0 {
                    self.ier |= value & 0x7F;
                } else {
                    self.ier &= !value;
                }
                self.update_irq();
            }
            0x0F => self.ora = value, // ORA (no handshake)
            _ => {}
        }
    }

    /// Update the IRQ status based on IFR and IER.
    fn update_irq(&mut self) {
        if (self.ifr & self.ier & 0x7F) != 0 {
            self.ifr |= 0x80;
            self.irq = true;
        } else {
            self.ifr &= 0x7F;
            self.irq = false;
        }
    }

    /// This method is called on each CPU cycle.
    pub fn tick_cycle(&mut self) {
        // --- Timer 1 ---
        let (next_t1c, overflow) = self.t1c.overflowing_sub(1);
        if overflow {
            if self.t1_active {
                self.ifr |= 0x40;
                self.t1_pb7 = !self.t1_pb7;
                if (self.acr & 0x40) != 0 {
                    self.t1c = self.t1l;
                } else {
                    self.t1_active = false;
                    self.t1c = 0xFFFF;
                }
                self.update_irq();
            } else {
                self.t1c = 0xFFFF;
            }
        } else {
            self.t1c = next_t1c;
        }

        // --- Timer 2 ---
        if (self.acr & 0x20) == 0 {
            let (next_t2c, overflow) = self.t2c.overflowing_sub(1);
            if overflow {
                if self.t2_active {
                    self.ifr |= 0x20;
                    self.t2_active = false;
                    self.update_irq();
                }
                self.t2c = 0xFFFF;
            } else {
                self.t2c = next_t2c;
            }
        }

        // --- Shift Register (Simplified) ---
        if self.sr_active {
            if self.sr_count > 0 {
                self.sr_count -= 1;
            } else {
                self.ifr |= 0x04;
                self.sr_active = false;
                self.update_irq();
            }
        }
    }

    /// Convenience function to get the value of CB1, if it an output
    #[inline]
    pub fn get_ca2_out(&self) -> Option<bool> {
        match self.pcr & 0b0000_1110 {
            0b0000_1100 => Option::Some(false),
            0b0000_1110 => Option::Some(true),
            _ => Option::None
        }
    }

    /// Convenience function to get the value of CB2, if it an output
    #[inline]
    pub fn get_cb2_out(&self) -> Option<bool> {
        match self.pcr & 0b1110_0000 {
            0b1100_0000 => Option::Some(false),
            0b1110_0000 => Option::Some(true),
            _ => Option::None
        }
    }

    /// Trigger the CA1 IRQ and Latch PA, regardless of the Edge configuration
    /// (Fast convenience method, but this is NOT exact behavior)
    #[inline]
    pub fn ca1_irq(&mut self) {
        self.ifr |= 0x02;
        self.update_irq();
        if (self.acr & 0x01) != 0 {
            self.latch_a = (self.ora & self.ddra) | (self.in_a & !self.ddra);
        }
    }

    /// Trigger the CA2 IRQ, regardless of the Edge configuration
    /// (Fast convenience method, but this is NOT exact behavior)
    #[inline]
    pub fn ca2_irq(&mut self) {
        self.ifr |= 0x01;
        self.update_irq();
    }


    pub fn set_ca1(&mut self, val: bool) {
        let positive_edge = (self.pcr & 0x01) != 0;
        if (val && !self.last_ca1 && positive_edge) || (!val && self.last_ca1 && !positive_edge) {
            self.ca1_irq();
        }
        self.last_ca1 = val;
    }

    pub fn set_ca2(&mut self, val: bool) {
        if (self.pcr & 0x08) == 0 {
            let positive_edge = (self.pcr & 0x04) != 0;
            if (val && !self.last_ca2 && positive_edge) || (!val && self.last_ca2 && !positive_edge) {
                self.ca2_irq();
            }
        }
        self.last_ca2 = val;
    }

    pub fn set_cb1(&mut self, val: bool) {
        let positive_edge = (self.pcr & 0x10) != 0;
        if (val && !self.last_cb1 && positive_edge) || (!val && self.last_cb1 && !positive_edge) {
            self.ifr |= 0x10;
            self.update_irq();
            if (self.acr & 0x02) != 0 {
                self.latch_b = (self.orb & self.ddrb) | (self.in_b & !self.ddrb);
            }
        }
        self.last_cb1 = val;
    }

    pub fn set_cb2(&mut self, val: bool) {
        if (self.pcr & 0x80) == 0 {
            let positive_edge = (self.pcr & 0x40) != 0;
            if (val && !self.last_cb2 && positive_edge) || (!val && self.last_cb2 && !positive_edge)
            {
                self.ifr |= 0x08;
                self.update_irq();
            }
        }
        self.last_cb2 = val;
    }

    pub fn get_port_b(&self) -> u8 {
        let mut out = (self.orb & self.ddrb) | (self.in_b & !self.ddrb);
        if (self.acr & 0x80) != 0 {
            if self.t1_pb7 {
                out |= 0x80;
            } else {
                out &= 0x7F;
            }
        }
        out
    }

    pub fn get_port_a(&self) -> u8 {
        (self.ora & self.ddra) | (self.in_a & !self.ddra)
    }
}

#[cfg(feature = "debug")]
impl MemoryMonitor for Via6522 {
    /// Peek at a VIA register without side effects.ge
    fn peek(&self, addr: u16) -> u8 {
        let register = (addr & 0x0F) as usize;
        match register {
            0x00 => {
                // IRB
                if (self.acr & 0x02) != 0 {
                    self.latch_b
                } else {
                    (self.orb & self.ddrb) | (self.in_b & !self.ddrb)
                }
            }
            0x01 => {
                // IRA
                if (self.acr & 0x01) != 0 {
                    self.latch_a
                } else {
                    (self.ora & self.ddra) | (self.in_a & !self.ddra)
                }
            }
            0x02 => self.ddrb,
            0x03 => self.ddra,
            0x04 => (self.t1c & 0xFF) as u8,
            0x05 => (self.t1c >> 8) as u8,
            0x06 => (self.t1l & 0xFF) as u8,
            0x07 => (self.t1l >> 8) as u8,
            0x08 => (self.t2c & 0xFF) as u8,
            0x09 => (self.t2c >> 8) as u8,
            0x0A => self.sr,
            0x0B => self.acr,
            0x0C => self.pcr,
            0x0D => self.ifr,
            0x0E => self.ier | 0x80,
            0x0F => {
                // IRA (no handshake)
                if (self.acr & 0x01) != 0 {
                    self.latch_a
                } else {
                    (self.ora & self.ddra) | (self.in_a & !self.ddra)
                }
            }
            _ => 0,
        }
    }

    fn poke(&mut self, _addr: u16, _value: u8) {
        // NOT implemented yet!
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer1_oneshot() {
        let mut via = Via6522::new();
        via.write(0x0E, 0xC0);
        via.write(0x04, 0x0A);
        via.write(0x05, 0x00);
        assert!(via.t1_active);
        for _ in 0..11 {
            via.tick_cycle();
        }
        assert!(via.ifr & 0x40 != 0);
        assert!(via.irq);
        assert!(!via.t1_active);
    }

    #[test]
    fn test_latching() {
        let mut via = Via6522::new();
        via.write(0x0B, 0x01);
        via.in_a = 0xAA;
        via.set_ca1(true);
        via.set_ca1(false);
        via.in_a = 0xFF;
        assert_eq!(via.read(0x01), 0xAA);
    }
}
