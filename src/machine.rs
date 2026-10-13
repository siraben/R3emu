//! Machine configuration is separate from the execution state and ROM contents.

use crate::bus::{Bus, TerminalPort};
use crate::vm::{CoreType, Vm};

#[derive(Clone, Debug)]
pub struct MachineConfig {
    pub memory_rows: u8,
    pub row_size: u16,
    pub cores: Vec<CoreType>,
    pub terminal_columns: u8,
    pub terminal_rows: u8,
    pub terminal_port: TerminalPort,
    pub terminate_unmapped: bool,
    pub pixel_plotter: bool,
    pub secondary_multiply: bool,
    pub initial_registers: [u32; 32],
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            memory_rows: 64,
            row_size: 128,
            cores: vec![CoreType::M; 10],
            terminal_columns: 12,
            terminal_rows: 8,
            terminal_port: TerminalPort::Last,
            terminate_unmapped: true,
            pixel_plotter: true,
            secondary_multiply: true,
            initial_registers: [0x2000_0000; 32],
        }
    }
}

impl MachineConfig {
    /// Physical configuration and saved register contents of CPS 3236906.
    /// This does not bundle the save's ROM or synthesize its desktop UI state.
    pub fn showcase() -> Self {
        Self {
            memory_rows: 16,
            cores: vec![
                CoreType::M,
                CoreType::S,
                CoreType::F,
                CoreType::F,
                CoreType::F,
                CoreType::M,
                CoreType::S,
                CoreType::F,
                CoreType::F,
                CoreType::F,
            ],
            initial_registers: std::array::from_fn(|i| 0x2000_0000 + i as u32),
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.memory_rows == 0 || self.row_size == 0 {
            return Err("memory dimensions must be nonzero");
        }
        if u16::from(self.memory_rows)
            .checked_mul(self.row_size)
            .is_none()
        {
            return Err("memory must fit in 65535 words");
        }
        if self.cores.is_empty() {
            return Err("CPU must have at least one execution unit");
        }
        if !(1..=29).contains(&self.terminal_columns) || !(1..=29).contains(&self.terminal_rows) {
            return Err("R316 terminals support 1 to 29 character rows and columns");
        }
        if let TerminalPort::Unit(unit) = self.terminal_port {
            if unit >= self.cores.len() {
                return Err("terminal unit is outside the CPU layout");
            }
        }
        Ok(())
    }

    pub fn build(&self, trace_size: Option<usize>) -> Result<Vm, &'static str> {
        self.validate()?;
        let bus = Bus::new(
            self.memory_rows,
            self.row_size,
            self.terminal_columns,
            self.terminal_rows,
            self.pixel_plotter,
        )
        .with_terminal_port(self.terminal_port)
        .with_termination(self.terminate_unmapped);
        let mut vm = Vm::new(bus, self.cores.clone(), self.secondary_multiply, trace_size);
        vm.initialize_registers(self.initial_registers);
        vm.bus.ram.fill(0x2000_0000);
        Ok(vm)
    }
}

pub fn parse_cores(layout: &str) -> Result<Vec<CoreType>, &'static str> {
    if layout.is_empty() {
        return Err("core layout must not be empty");
    }
    layout
        .bytes()
        .map(|byte| match byte.to_ascii_lowercase() {
            b'm' => Ok(CoreType::M),
            b's' => Ok(CoreType::S),
            b'f' => Ok(CoreType::F),
            _ => Err("core layout may contain only M, S and F"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showcase_profile_matches_the_saved_topology_and_registers() {
        let vm = MachineConfig::showcase().build(None).unwrap();
        assert_eq!(vm.bus.ram.len(), 2048);
        assert_eq!(vm.cores, parse_cores("msfffmsfff").unwrap());
        assert_eq!(vm.registers()[0], 0x2000_0000);
        assert_eq!(vm.registers()[31], 0x2000_001F);
    }

    #[test]
    fn invalid_layouts_and_ports_are_rejected_before_execution() {
        assert!(parse_cores("").is_err());
        assert!(parse_cores("mx").is_err());
        let config = MachineConfig {
            terminal_port: TerminalPort::Unit(10),
            ..MachineConfig::default()
        };
        assert!(config.build(None).is_err());
        assert_eq!(
            parse_cores("MsF").unwrap(),
            [CoreType::M, CoreType::S, CoreType::F]
        );
    }
}
