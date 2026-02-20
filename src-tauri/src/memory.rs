//! Memory abstraction with bounds checks, regions, and little-endian read/write.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    data: Vec<u8>,
    size: usize,
}

impl Memory {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
            size,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    fn check_bounds(&self, addr: u32, len: usize) -> Result<(), String> {
        let addr = addr as usize;
        if addr + len > self.size {
            Err(format!(
                "Memory access out of bounds: 0x{:08X} + {} > 0x{:08X}",
                addr, len, self.size
            ))
        } else {
            Ok(())
        }
    }

    pub fn read_u8(&self, addr: u32) -> Result<u8, String> {
        self.check_bounds(addr, 1)?;
        Ok(self.data[addr as usize])
    }

    pub fn read_u16_le(&self, addr: u32) -> Result<u16, String> {
        self.check_bounds(addr, 2)?;
        let a = addr as usize;
        Ok(u16::from_le_bytes([self.data[a], self.data[a + 1]]))
    }

    pub fn write_u16_le(&mut self, addr: u32, value: u16) -> Result<[u8; 2], String> {
        self.check_bounds(addr, 2)?;
        let bytes = value.to_le_bytes();
        let a = addr as usize;
        let old = [self.data[a], self.data[a + 1]];
        self.data[a..a + 2].copy_from_slice(&bytes);
        Ok(old)
    }

    pub fn write_u8(&mut self, addr: u32, value: u8) -> Result<u8, String> {
        self.check_bounds(addr, 1)?;
        let old = self.data[addr as usize];
        self.data[addr as usize] = value;
        Ok(old)
    }

    pub fn read_u32_le(&self, addr: u32) -> Result<u32, String> {
        self.check_bounds(addr, 4)?;
        let a = addr as usize;
        Ok(u32::from_le_bytes([
            self.data[a],
            self.data[a + 1],
            self.data[a + 2],
            self.data[a + 3],
        ]))
    }

    pub fn write_u32_le(&mut self, addr: u32, value: u32) -> Result<[u8; 4], String> {
        self.check_bounds(addr, 4)?;
        let bytes = value.to_le_bytes();
        let a = addr as usize;
        let old = [self.data[a], self.data[a + 1], self.data[a + 2], self.data[a + 3]];
        self.data[a..a + 4].copy_from_slice(&bytes);
        Ok(old)
    }

    /// Load program bytes at given address (e.g. entry point)
    pub fn load_program(&mut self, at_addr: u32, bytes: &[u8]) -> Result<(), String> {
        self.check_bounds(at_addr, bytes.len())?;
        let start = at_addr as usize;
        self.data[start..start + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}
