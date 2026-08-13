//! A functional implementation of the MOS 6522 VIA.
//!
//! This module provides a detailed simulation of the 6522 VIA,
//! including timers, interrupts, and I/O ports.
//! However, the functionality is simplified to cover what is actually
//! used by the 1541 hardware, and some features (like the shift register) 
//! is simplified to only fire interrupts without actually shifting data.

#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
#[cfg(feature = "debug")]
use crate::MemoryMonitor;

#[derive(Clone, Copy, Default)]
pub struct Via6522 {
    orb: u8,
    ora: u8,
    ddrb: u8,
    ddra: u8,
    t1c: u16,
    t1l: u16,
    t1_pb7: bool,
    t2c: u16,
    t2l: u8,
    sr: u8,
    acr: u8,
    pcr: u8,
    ifr: u8,
    ier: u8,
    in_a: u8,
    in_b: u8,
    irq: bool,
    latch_a: u8,
    latch_b: u8,
    last_ca1: bool,
    last_cb1: bool,
    last_ca2: bool,
    last_cb2: bool,
    sr_count: u8,
    active_mask: u8,
}

impl Via6522 {
    pub const T1_ACTIVE: u8 = 0x01;
    pub const T2_ACTIVE: u8 = 0x02;
    pub const SR_ACTIVE: u8 = 0x04;
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

    #[inline(always)]
    pub fn has_irq(&self) -> bool { self.irq }

    #[inline(always)]
    pub fn set_pa_in(&mut self, val: u8) { self.in_a = val; }

    #[inline(always)]
    pub fn set_pb_in(&mut self, val: u8) { self.in_b = val; }

    #[inline(always)]
    pub fn pb_in(&self) -> u8 { self.in_b }

    #[inline(always)]
    pub fn pa_in(&self) -> u8 { self.in_a }

    #[inline(always)]
    pub fn pcr(&self) -> u8 { self.pcr }

    #[inline(always)]
    pub fn ora(&self) -> u8 { self.ora }

    #[inline(always)]
    pub fn active_mask(&self) -> u8 { self.active_mask }

    #[inline(always)]
    pub fn t1c(&self) -> u16 { self.t1c }

    #[inline(always)]
    pub fn t2c(&self) -> u16 { self.t2c }

    #[inline(always)]
    pub fn sr_count(&self) -> u8 { self.sr_count }

    /// Read a byte from a VIA register.
    
    #[inline(always)]
    #[link_section = ".data.ram_func"]
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
                    let pb = self.in_b & self.in_b;
                    (self.orb & self.ddrb) | (pb & !self.ddrb)
                }
            }
            0x01 => {
                // IRA
                self.ifr &= !0x03; // Clear CA1 and CA2 interrupt flags
                self.update_irq();
                if (self.acr & 0x01) != 0 {
                    self.latch_a
                } else {
                    let pa = self.in_a & self.in_a;
                    (self.ora & self.ddra) | (pa & !self.ddra)
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
                    let pa = self.in_a & self.in_a;
                    (self.ora & self.ddra) | (pa & !self.ddra)
                }
            }
            _ => 0,
        }
    }

    /// Write a byte to a VIA register.
    #[inline(always)]
    #[link_section = ".data.ram_func"]
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
            0x02 => {
                self.ddrb = value;
            }
            0x03 => {
                self.ddra = value;
            }
            0x04 => self.t1l = (self.t1l & 0xFF00) | (value as u16), // T1L-L
            0x05 => {
                // T1C-H (starts T1)
                self.t1l = (self.t1l & 0x00FF) | ((value as u16) << 8);
                self.t1c = self.t1l;
                self.active_mask |= Self::T1_ACTIVE;
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
                self.active_mask |= Self::T2_ACTIVE;
                self.ifr &= !0x20; // Clear T2 interrupt flag
                self.update_irq();
            }
            0x0A => {
                // SR
                self.sr = value;
                self.ifr &= !0x04; // Clear SR interrupt flag
                self.update_irq();
                if (self.acr & 0x1C) != 0 {
                    self.active_mask |= Self::SR_ACTIVE;
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
            0x0F => {
                self.ora = value; // ORA (no handshake)
            }
            _ => {}
        }
    }

    /// Update the IRQ status based on IFR and IER.
    
    #[inline(always)]
    #[link_section = ".data.ram_func"]
    fn update_irq(&mut self) {
        let pending = self.ifr & self.ier & 0x7F;
        self.irq = pending != 0;
        let irq_bit = (self.irq as u8) << 7;
        self.ifr = (self.ifr & 0x7F) | irq_bit;
    }

    /// This method is called on each CPU cycle.
    
    #[inline(always)]
    #[link_section = ".data.ram_func"]
    pub fn any_active(&self) -> bool {
        self.active_mask != 0
    }

    /// This method is called on each CPU cycle.
    
    #[inline(always)]
    #[link_section = ".data.ram_func"]
    pub fn tick_cycles(&mut self, delta: u32) {
        if delta == 0 || self.active_mask == 0 { return; }
        let delta16 = delta as u16;

        match self.active_mask {
            1 => {
                if self.t1c < delta16 {
                    self.ifr |= 0x40;
                    self.t1_pb7 = !self.t1_pb7;
                    if (self.acr & 0x40) != 0 {
                        self.t1c = self.t1l.wrapping_sub(delta16 - self.t1c - 1);
                    } else {
                        self.active_mask &= !Self::T1_ACTIVE;
                        self.t1c = 0xFFFF;
                    }
                    self.update_irq();
                } else {
                    self.t1c -= delta16;
                }
            },
            _ => {
                // --- Timer 1 ---
                if (self.active_mask & Self::T1_ACTIVE) != 0 {
                    if self.t1c < delta16 {
                        self.ifr |= 0x40;
                        self.t1_pb7 = !self.t1_pb7;
                        if (self.acr & 0x40) != 0 {
                            let remainder = delta16 - self.t1c - 1;
                            self.t1c = self.t1l.wrapping_sub(remainder);
                        } else {
                            self.active_mask &= !Self::T1_ACTIVE;
                            self.t1c = 0xFFFF;
                        }
                        self.update_irq();
                    } else {
                        self.t1c -= delta16;
                    }
                }

                // --- Timer 2 ---
                if (self.active_mask & Self::T2_ACTIVE) != 0 && (self.acr & 0x20) == 0 {
                    if self.t2c < delta16 {
                        self.ifr |= 0x20;
                        self.active_mask &= !Self::T2_ACTIVE;
                        self.update_irq();
                        self.t2c = 0xFFFF;
                    } else {
                        self.t2c -= delta16;
                    }
                }

                // --- Shift Register (Simplified) ---
                if (self.active_mask & Self::SR_ACTIVE) != 0 {
                    let delta8 = delta16 as u8;
                    if self.sr_count >= delta8 {
                        self.sr_count -= delta8;
                    } else {
                        self.sr_count = 0;
                        self.ifr |= 0x04;
                        self.active_mask &= !Self::SR_ACTIVE;
                        self.update_irq();
                    }
                }
            }
        }
    }

    /// Convenience function to get the value of PB, masked by the direction register.
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn get_pb_out(&self) -> u8 {
        self.orb & self.ddrb
    }

    /// Convenience function to get the value of PA, masked by the direction register.
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn get_pa_out(&self) -> u8 {
        self.ora & self.ddra
    }

    /// Convenience function to get the value of CB1, if it an output
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn get_ca2_out(&self) -> Option<bool> {
        match self.pcr & 0b0000_1110 {
            0b0000_1100 => Option::Some(false),
            0b0000_1110 => Option::Some(true),
            0b0000_1010 => Option::Some(true), // Pulse output (goes low on read, high on CA1)
            _ => Option::None
        }
    }

    /// Convenience function to get the value of CB2, if it an output
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn get_cb2_out(&self) -> Option<bool> {
        match self.pcr & 0b1110_0000 {
            0b1100_0000 => Option::Some(false),
            0b1110_0000 => Option::Some(true),
            _ => Option::None
        }
    }

    /// Trigger the CA1 IRQ and Latch PA, regardless of the Edge configuration
    /// (Fast convenience method, but this is NOT exact behavior)
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn ca1_irq(&mut self) {
        self.ifr |= 0x02;
        self.update_irq();
        if (self.acr & 0x01) != 0 {
            self.latch_a = (self.ora & self.ddra) | (self.in_a & self.in_a & !self.ddra);
        }
    }

    /// Trigger the CA2 IRQ, regardless of the Edge configuration
    /// (Fast convenience method, but this is NOT exact behavior)
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn ca2_irq(&mut self) {
        self.ifr |= 0x01;
        self.update_irq();
    }

    /// Trigger the CA1 edge IRQ, when we already know that the edge has changed
    /// (Fast convenience method, but this is NOT exact behavior)
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn ca1_edge(&mut self, edge: bool) {
        self.last_ca1 = edge;
        let positive_edge = (self.pcr & 0x01) != 0;
        
        if edge == positive_edge {
            self.ca1_irq();
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn set_ca1(&mut self, val: bool) {
        let positive_edge = (self.pcr & 0x01) != 0;
        if (val && !self.last_ca1 && positive_edge) || (!val && self.last_ca1 && !positive_edge) {
            self.ca1_irq();
        }
        self.last_ca1 = val;
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn set_ca2(&mut self, val: bool) {
        if (self.pcr & 0x08) == 0 {
            let positive_edge = (self.pcr & 0x04) != 0;
            if (val && !self.last_ca2 && positive_edge) || (!val && self.last_ca2 && !positive_edge) {
                self.ca2_irq();
            }
        }
        self.last_ca2 = val;
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
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

    #[link_section = ".data.ram_func"]
    #[inline(always)]
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
}

#[cfg(feature = "debug")]
impl MemoryMonitor for Via6522 {
    /// Peek at a VIA register without side effects.ge
    fn peek<H: ViaHardware>(&mut self, addr: u16, _hw: &mut H) -> u8 {
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

    fn poke<H: ViaHardware>(&mut self, _addr: u16, _value: u8, _hw: &mut H) {
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
        assert!(via.active_mask & Via6522::T1_ACTIVE != 0);
        for _ in 0..11 {
            via.tick_cycles(1);
        }
        assert!(via.ifr & 0x40 != 0);
        assert!(via.irq);
        assert!(via.active_mask & Via6522::T1_ACTIVE == 0);
    }

    #[test]
    fn test_latching() {
        let mut via = Via6522::new();
        via.write(0x0B, 0x01);
        via.in_a = 0xAA;
        via.ca1_irq();
        via.in_a = 0xFF;
        assert_eq!(via.read(0x01), 0xAA);
    }
}
