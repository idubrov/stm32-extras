//! Extra API on top of STM32 device crates (`stm32f103xx`)
//!
//! # Examples
//!
//! Configuring GPIO pins without disturbing other pins (no read-modify-write which could lead to
//! data races):
//!
//! ```rust,no_run
//! # extern crate stm32_extras;
//! # extern crate stm32f103xx;
//! use stm32_extras::GPIOExtras;
//! # fn main() {
//! let gpioc = unsafe { &*stm32f103xx::GPIOC.get() }; // Get GPIOC somehow...
//!
//! // Set pin to 2Mhz, open-drain.
//! // Modifies corresponding GPIO configuration bits without reads
//! gpioc.pin_config(13).output2().open_drain();
//! # }
//! ```
//!
//! Generalized interface to GPIO pins:
//!
//! ```rust,no_run
//! # extern crate stm32_extras;
//! # extern crate stm32f103xx;
//! use stm32_extras::GPIOExtras;
//! # fn main() {
//! let gpioc = unsafe { &*stm32f103xx::GPIOC.get() }; // Get GPIOC somehow...
//!
//! // Set pins 13, 14 and 15 on GPIOC to 1, 0 and 1.
//! gpioc.write_pin_range(13, 3, 0b101);
//! # }
//! ```
#![deny(missing_docs)]
#![deny(warnings)]
#![no_std]

/// Convenient access to the bit blocks on GPIO ports.
pub trait GPIOExtras<T> {
    /// Set `count` bits on the GPIO port starting from the bit number `offset`. Other bits are not
    /// affected. Uses BSRR register to set/clear individual bits.
    /// Bits must fit into 16 bits of the GPIO port.
    fn write_pin_range(&self, offset: usize, count: usize, data: u16);

    /// Set single bit at the given `offset` in GPIO port. `offset` must be in the range 0..16.
    fn write_pin(&self, offset: usize, bit: bool) {
        self.write_pin_range(offset, 1, if bit { 1 } else { 0 });
    }

    /// Get `count` bits on the GPIO port starting from the bit number `offset`.
    fn read_pin_range(&self, offset: usize, count: usize) -> u16;

    /// Get single bit at the given `offset` in GPIO port. `offset` must be in the range 0..16.
    fn read_pin(&self, offset: usize) -> bool {
        self.read_pin_range(offset, 1) != 0
    }

    /// Get access to configuration bits for `pin` of GPIO port.
    fn pin_config(&self, pin: usize) -> &T;
}

/// Common features for STM32F1/STM32W1 series.
#[cfg(feature = "stm32f103xx")]
mod stm32f1xx {
    extern crate vcell;
    use self::vcell::VolatileCell;

    /// Pin configuration registers for STM32F1/STM32W1
    pub struct GPIOBitbandConfigBlock {
        mode_low: VolatileCell<u32>,
        mode_high: VolatileCell<u32>,
        cnf_low: VolatileCell<u32>,
        cnf_high: VolatileCell<u32>,
    }

    impl GPIOBitbandConfigBlock {
        /// Input mode (reset state)
        pub fn input(&self) -> &Self {
            self.mode_low.set(0);
            self.mode_high.set(0);
            self
        }

        /// Output mode, max speed 2 MHz.
        pub fn output2(&self) -> &Self {
            self.mode_low.set(0);
            self.mode_high.set(1);
            self
        }

        /// Output mode, max speed 10 MHz.
        pub fn output10(&self) -> &Self {
            self.mode_low.set(1);
            self.mode_high.set(0);
            self
        }

        /// Output mode, max speed 50 MHz.
        pub fn output50(&self) -> &Self {
            self.mode_low.set(1);
            self.mode_high.set(1);
            self
        }

        // Output config

        /// Push-pull
        pub fn push_pull(&self) -> &Self {
            self.cnf_low.set(0);
            self
        }

        /// Open-drain
        pub fn open_drain(&self) -> &Self {
            self.cnf_low.set(1);
            self
        }

        /// General purpose
        pub fn general(&self) -> &Self {
            self.cnf_high.set(0);
            self
        }

        /// Alternate function
        pub fn alternate(&self) -> &Self {
            self.cnf_high.set(1);
            self
        }

        // Input config

        /// Analog mode
        pub fn analog(&self) -> &Self {
            self.cnf_low.set(0);
            self.cnf_high.set(0);
            self
        }

        /// Floating input (reset state)
        pub fn floating(&self) -> &Self {
            // Ordering is important: should never get reserved value of `11`
            self.cnf_high.set(0);
            self.cnf_low.set(1);
            self
        }

        /// Input with pull-up / pull-down
        pub fn pull_up_down(&self) -> &Self {
            self.cnf_low.set(0);
            self.cnf_high.set(1);
            self
        }
    }

    /// GPIO port configuration bits
    #[repr(C)]
    pub struct GPIOBitbandRegisterBlock {
        config: [GPIOBitbandConfigBlock; 16],
    }

    impl GPIOBitbandRegisterBlock {
        /// Get pin configuration bits
        pub fn config(&self, pin: usize) -> &GPIOBitbandConfigBlock {
            &self.config[pin]
        }
    }
}

#[cfg(feature = "stm32f103xx")]
mod stm32f103 {
    extern crate stm32f103xx;
    use self::stm32f103xx::gpioa;
    use super::stm32f1xx::GPIOBitbandRegisterBlock;
    use super::stm32f1xx::GPIOBitbandConfigBlock;
    use super::GPIOExtras;

    const PERIPHERALS_BASE: usize = 0x4000_0000;
    const PERIPHERALS_ALIAS: usize = 0x4200_0000;

    fn to_bitband_address<S, T>(port: &T) -> &'static S {
        let byte_offset = (port as *const T as usize) - PERIPHERALS_BASE;
        let address = PERIPHERALS_ALIAS + byte_offset * 32;
        let ptr = address as *const S;
        unsafe { &*ptr }
    }

    impl GPIOExtras<GPIOBitbandConfigBlock> for gpioa::RegisterBlock {
        fn write_pin_range(&self, offset: usize, count: usize, data: u16) {
            let mask = (1 << count) - 1;
            let bits = u32::from(data & mask) | // Set '1's
                (u32::from(!data & mask) << 16); // Clear '0's
            self.bsrr.write(|w| unsafe { w.bits(bits << offset) });
        }

        fn read_pin_range(&self, offset: usize, count: usize) -> u16 {
            let mask = (1 << count) - 1;
            ((self.idr.read().bits() >> offset) as u16) & mask
        }

        fn pin_config(&self, pin: usize) -> &GPIOBitbandConfigBlock {
            let registers: &GPIOBitbandRegisterBlock = to_bitband_address(self);
            &registers.config(pin)
        }
    }
}