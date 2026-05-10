//! device/serial.rs -- 16550A UART emulation (modelo QEMU serial_update_irq)

use std::io::Write;
use std::sync::Arc;
use vmm_sys_util::eventfd::EventFd;

pub const SERIAL_BASE: u16 = 0x3f8;

// LSR bits
const LSR_DR:   u8 = 0x01;  // Data Ready
const LSR_THRE: u8 = 0x20;  // Transmitter Holding Register Empty
const LSR_TEMT: u8 = 0x40;  // Transmitter Empty

// IER bits
const IER_RDI:  u8 = 0x01;  // Receiver Data Interrupt
const IER_THRI: u8 = 0x02;  // Transmitter Holding Register Empty Interrupt

// IIR values
const IIR_NO_INT: u8 = 0x01;  // No interrupt pending
const IIR_THRI:   u8 = 0x02;  // TX holding register empty
const IIR_RDI:    u8 = 0x04;  // Receiver data available

pub struct Serial {
    pub rbr:    u8,   // Receiver Buffer Register
    pub ier:    u8,   // Interrupt Enable Register
    pub iir:        u8,   // Interrupt Identification Register
    lcr:        u8,   // Line Control Register
    mcr:        u8,   // Modem Control Register
    pub lsr:    u8,   // Line Status Register
    msr:        u8,   // Modem Status Register
    scr:        u8,   // Scratch Register
    dll:        u8,   // Divisor Latch LSB
    dlm:        u8,   // Divisor Latch MSB
    dlab:       bool, // Divisor Latch Access Bit
    output:     Vec<u8>,
    pub irqfd:  Option<Arc<EventFd>>,
}

impl Serial {
    pub fn new() -> Self {
        Self {
            rbr: 0,
            ier: 0,
            iir: IIR_NO_INT,
            lcr: 0,
            mcr: 0,
            lsr: LSR_THRE | LSR_TEMT,
            msr: 0,
            scr: 0,
            dll: 0,
            dlm: 0,
            dlab: false,
            output: Vec::new(),
            irqfd: None,
        }
    }

    // QEMU-style: recalculate IIR and inject/clear IRQ
    pub fn update_irq(&mut self) {
        let old_iir = self.iir;
        if (self.ier & IER_THRI) != 0 && (self.lsr & LSR_THRE) != 0 {
            self.iir = IIR_THRI;
        } else if (self.ier & IER_RDI) != 0 && (self.lsr & LSR_DR) != 0 {
            self.iir = IIR_RDI;
        } else {
            self.iir = IIR_NO_INT;
        }
        // Inject IRQ if pending
        if self.iir != IIR_NO_INT {
            if let Some(ref fd) = self.irqfd {
                let _ = fd.write(1);
            }
        }
    }

    pub fn write_port(&mut self, port: u16, data: u8) {
        let reg = port - SERIAL_BASE;
        match reg {
            0 => {
                if self.dlab {
                    self.dll = data;
                } else {
                    // THR write -- transmit byte
                    self.transmit_byte(data);
                    // LSR: THRE clear while transmitting, then set again
                    self.lsr &= !LSR_THRE;
                    self.lsr &= !LSR_TEMT;
                    // Immediately ready again (we flush instantly)
                    self.lsr |= LSR_THRE | LSR_TEMT;
                    self.update_irq();
                }
            }
            1 => {
                if self.dlab {
                    self.dlm = data;
                } else {
                    self.ier = data;
                    self.update_irq();
                }
            }
            2 => { /* FCR -- ignore FIFO control */ }
            3 => { self.lcr = data; self.dlab = data & 0x80 != 0; }
            4 => { self.mcr = data; }
            7 => { self.scr = data; }
            _ => {}
        }
    }

    pub fn read_port(&mut self, port: u16) -> u8 {
        let reg = port - SERIAL_BASE;
        match reg {
            0 => {
                if self.dlab { self.dll }
                else {
                    let v = self.rbr;
                    self.lsr &= !LSR_DR;
                    self.update_irq();
                    v
                }
            }
            1 => if self.dlab { self.dlm } else { self.ier },
            2 => {
                // Reading IIR clears THRI interrupt
                let v = self.iir;
                if v == IIR_THRI { self.iir = IIR_NO_INT; }
                v
            }
            3 => self.lcr,
            4 => self.mcr,
            5 => self.lsr,
            6 => self.msr,
            7 => self.scr,
            _ => 0xFF,
        }
    }

    fn transmit_byte(&mut self, b: u8) {
        self.output.push(b);
        if b == b'\n' || self.output.len() >= 64 {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        if !self.output.is_empty() {
            let _ = std::io::stdout().write_all(&self.output);
            let _ = std::io::stdout().flush();
            self.output.clear();
        }
    }
}
