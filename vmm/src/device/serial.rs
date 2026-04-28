//! vmm/src/device/serial.rs — UART 16550A emulación
//!
//! Implementa el UART 16550A que el kernel de Linux usa como consola
//! serie (ttyS0). El kernel escribe caracteres via outb al puerto 0x3F8
//! y los leemos aquí, imprimiéndolos en stdout del proceso VMM.
//!
//! Registros 16550A (base = 0x3F8 para COM1):
//!   0x3F8  RBR/THR/DLL  Receive Buffer / Transmit Holding / Divisor LSB
//!   0x3F9  IER/DLM      Interrupt Enable / Divisor MSB
//!   0x3FA  IIR/FCR      Interrupt ID / FIFO Control
//!   0x3FB  LCR          Line Control Register
//!   0x3FC  MCR          Modem Control Register
//!   0x3FD  LSR          Line Status Register (THRE bit = listo para TX)
//!   0x3FE  MSR          Modem Status Register
//!   0x3FF  SCR          Scratch Register

use std::io::Write;
use anyhow::Result;

pub const SERIAL_BASE: u16 = 0x3F8;  // COM1
pub const SERIAL_SIZE: u16 = 8;

// LSR bits
const LSR_THRE:  u8 = 0x20;  // Transmitter Holding Register Empty (listo para TX)
const LSR_TEMT:  u8 = 0x40;  // Transmitter Empty
const LSR_DR:    u8 = 0x01;  // Data Ready (hay datos para leer)

pub struct Serial {
    rbr:  u8,     // Receive Buffer Register
    ier:  u8,     // Interrupt Enable Register
    iir:  u8,     // Interrupt ID Register
    lcr:  u8,     // Line Control Register
    mcr:  u8,     // Modem Control Register
    lsr:  u8,     // Line Status Register
    msr:  u8,     // Modem Status Register
    scr:  u8,     // Scratch Register
    dll:  u8,     // Divisor Latch LSB (cuando DLAB=1)
    dlm:  u8,     // Divisor Latch MSB (cuando DLAB=1)
    dlab: bool,   // Divisor Latch Access Bit (LCR bit 7)
    output: Vec<u8>, // buffer para el output hacia la consola
}

impl Serial {
    pub fn new() -> Self {
        Self {
            rbr: 0, ier: 0,
            iir: 0x01,    // no interrupt pending
            lcr: 0, mcr: 0,
            lsr: LSR_THRE | LSR_TEMT, // listo para transmitir
            msr: 0x30,    // CTS + DSR
            scr: 0,
            dll: 0x0C, dlm: 0, // 9600 baud @ 1.8432 MHz
            dlab: false,
            output: Vec::with_capacity(256),
        }
    }

    /// Manejar una escritura del guest al puerto I/O del serial.
    pub fn write_port(&mut self, port: u16, data: u8) {
        let reg = port - SERIAL_BASE;
        match reg {
            0 => {
                if self.dlab {
                    self.dll = data;  // Divisor Latch LSB
                } else {
                    // THR — Transmit Holding Register: el guest está enviando un byte
                    self.transmit_byte(data);
                }
            }
            1 => {
                if self.dlab {
                    self.dlm = data;  // Divisor Latch MSB
                } else {
                    self.ier = data;  // Interrupt Enable Register
                }
            }
            2 => { /* FCR write — ignoramos FIFO control */ }
            3 => {
                self.lcr  = data;
                self.dlab = data & 0x80 != 0;
            }
            4 => { self.mcr = data; }
            7 => { self.scr = data; }
            _ => {}
        }
    }

    /// Manejar una lectura del guest desde el puerto I/O del serial.
    pub fn read_port(&mut self, port: u16) -> u8 {
        let reg = port - SERIAL_BASE;
        match reg {
            0 => {
                if self.dlab { self.dll }
                else {
                    let v = self.rbr;
                    self.lsr &= !LSR_DR;  // limpiar data-ready
                    v
                }
            }
            1 => if self.dlab { self.dlm } else { self.ier }
            2 => self.iir,
            3 => self.lcr
            4 => self.mcr,
            5 => self.lsr
            6 => self.msr,
            7 => self.scr
            _ => 0xFF
        }
    }

    fn transmit_byte(&mut self, b: u8) {
        self.output.push(b);

        // Flush en newline o cuando el buffer está lleno
        if b == b'\n' || self.output.len() >= 256 {
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
