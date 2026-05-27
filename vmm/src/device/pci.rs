//! device/pci.rs -- PCI host bridge (config mechanism #1) + virtio-blk device.
//
// Config access: CONFIG_ADDRESS at 0xCF8 (a 32-bit latch) and CONFIG_DATA at
// 0xCFC. The kernel detects "type 1" PCI by writing 0x80000000 to 0xCF8 and
// reading it back, then walks the bus reading vendor IDs. Absent devices must
// read back 0xFFFFFFFF.
//
// Devices on bus 0:
//   00:00.0  Intel 440FX host bridge (class 0x06)
//   00:01.0  virtio-blk modern (1af4:1042) -- visible to enumeration only for
//            now. No capabilities list, no BARs, no MSI-X yet; those are the
//            next sub-milestones that make the guest's virtio-pci driver bind.
//
// The bridge is shared as Arc<Mutex<..>> and threaded through Vcpu::run.

pub const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
pub const PCI_CONFIG_DATA: u16 = 0xCFC;

struct PciDevice {
    config: [u8; 256],
}

impl PciDevice {
    fn host_bridge() -> Self {
        let mut config = [0u8; 256];
        // Vendor 0x8086 (Intel), Device 0x1237 (440FX), little-endian.
        config[0x00] = 0x86;
        config[0x01] = 0x80;
        config[0x02] = 0x37;
        config[0x03] = 0x12;
        config[0x08] = 0x02; // revision
        // Class code: prog-if 0x00, subclass 0x00 (host bridge), class 0x06.
        config[0x09] = 0x00;
        config[0x0a] = 0x00;
        config[0x0b] = 0x06;
        config[0x0e] = 0x00; // header type 0
        Self { config }
    }

    fn virtio_blk() -> Self {
        let mut config = [0u8; 256];
        // Vendor 0x1AF4 (Red Hat / virtio), Device 0x1042 (virtio-blk, modern).
        config[0x00] = 0xF4;
        config[0x01] = 0x1A;
        config[0x02] = 0x42;
        config[0x03] = 0x10;
        config[0x04] = 0x07; // command: IO + MEM + bus master
        config[0x06] = 0x10; // status: capabilities list present (bit 4)
        config[0x08] = 0x01; // revision (>= 1 = modern virtio)
        // Class code: prog-if 0x00, subclass 0x00, class 0x01 (mass storage).
        config[0x09] = 0x00;
        config[0x0a] = 0x00;
        config[0x0b] = 0x01;
        config[0x0e] = 0x00; // header type 0
        // Subsystem vendor/device (some guests check these): 0x1AF4 / 0x0002.
        config[0x2c] = 0xF4;
        config[0x2d] = 0x1A;
        config[0x2e] = 0x02;
        config[0x2f] = 0x00;
        // Interrupt pin INTA (cosmetic for now; no routing yet).
        config[0x3d] = 0x01;
        Self { config }
    }

    fn read(&self, offset: usize, data: &mut [u8]) {
        for (i, b) in data.iter_mut().enumerate() {
            let o = offset + i;
            *b = if o < 256 { self.config[o] } else { 0xFF };
        }
    }

    fn write(&mut self, offset: usize, data: &[u8]) {
        // Config is read-only for now; BAR writes come in the next sub-milestone.
        let _ = (offset, data);
    }
}

pub struct PciHostBridge {
    config_address: u32,
    host_bridge: PciDevice,
    virtio_blk: PciDevice,
}

impl PciHostBridge {
    pub fn new() -> Self {
        Self {
            config_address: 0,
            host_bridge: PciDevice::host_bridge(),
            virtio_blk: PciDevice::virtio_blk(),
        }
    }

    fn decode(&self) -> (bool, u8, u8, u8, usize) {
        let a = self.config_address;
        let enabled = (a & 0x8000_0000) != 0;
        let bus = ((a >> 16) & 0xFF) as u8;
        let dev = ((a >> 11) & 0x1F) as u8;
        let func = ((a >> 8) & 0x07) as u8;
        let reg = (a & 0xFC) as usize; // dword-aligned register offset
        (enabled, bus, dev, func, reg)
    }

    fn device_at(&self, bus: u8, dev: u8, func: u8) -> Option<&PciDevice> {
        match (bus, dev, func) {
            (0, 0, 0) => Some(&self.host_bridge),
            (0, 1, 0) => Some(&self.virtio_blk),
            _ => None,
        }
    }

    fn device_at_mut(&mut self, bus: u8, dev: u8, func: u8) -> Option<&mut PciDevice> {
        match (bus, dev, func) {
            (0, 0, 0) => Some(&mut self.host_bridge),
            (0, 1, 0) => Some(&mut self.virtio_blk),
            _ => None,
        }
    }

    pub fn write_port(&mut self, port: u16, data: &[u8]) {
        if (PCI_CONFIG_ADDRESS..PCI_CONFIG_ADDRESS + 4).contains(&port) {
            let mut bytes = self.config_address.to_le_bytes();
            for (i, b) in data.iter().enumerate() {
                let idx = (port - PCI_CONFIG_ADDRESS) as usize + i;
                if idx < 4 {
                    bytes[idx] = *b;
                }
            }
            self.config_address = u32::from_le_bytes(bytes);
            return;
        }
        if (PCI_CONFIG_DATA..PCI_CONFIG_DATA + 4).contains(&port) {
            let (enabled, bus, dev, func, reg) = self.decode();
            if !enabled {
                return;
            }
            let byte_off = reg + (port - PCI_CONFIG_DATA) as usize;
            if let Some(d) = self.device_at_mut(bus, dev, func) {
                d.write(byte_off, data);
            }
        }
    }

    pub fn read_port(&self, port: u16, data: &mut [u8]) {
        if (PCI_CONFIG_ADDRESS..PCI_CONFIG_ADDRESS + 4).contains(&port) {
            let bytes = self.config_address.to_le_bytes();
            for (i, b) in data.iter_mut().enumerate() {
                let idx = (port - PCI_CONFIG_ADDRESS) as usize + i;
                *b = if idx < 4 { bytes[idx] } else { 0 };
            }
            return;
        }
        if (PCI_CONFIG_DATA..PCI_CONFIG_DATA + 4).contains(&port) {
            let (enabled, bus, dev, func, reg) = self.decode();
            if !enabled {
                data.fill(0xFF);
                return;
            }
            let byte_off = reg + (port - PCI_CONFIG_DATA) as usize;
            match self.device_at(bus, dev, func) {
                Some(d) => d.read(byte_off, data),
                None => data.fill(0xFF), // absent device -> all ones
            }
        }
    }
}
