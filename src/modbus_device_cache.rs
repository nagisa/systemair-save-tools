use crate::registers::{RegisterIndex, Value};

pub struct RegisterBitmask([u64; u16::MAX as usize / u64::BITS as usize]);

impl RegisterBitmask {
    pub fn new() -> Self {
        Self([0; _])
    }

    fn word_bit_mask(&self, address: u16) -> (usize, u64) {
        let word = (u32::from(address) / u64::BITS) as usize;
        let bit_mask = 1 << u64::from(u32::from(address) % u64::BITS);
        (word, bit_mask)
    }

    pub fn is_set(&self, address: u16) -> bool {
        let (word, bitmask) = self.word_bit_mask(address);
        (self.0[word] & bitmask) != 0
    }

    pub fn set(&mut self, address: u16) {
        let (word, bitmask) = self.word_bit_mask(address);
        self.0[word] |= bitmask;
    }

    // Returns `true` only if `superset` has all bits from `self` set.
    pub fn is_subset_of(&self, superset: &RegisterBitmask) -> bool {
        for (&our, &their) in self.0.iter().zip(superset.0.iter()) {
            if (their & our) != our {
                return false;
            }
        }
        return true;
    }

    /// Finds an optimal list of ranges of set bits using dynamic programming.
    pub fn find_optimal_ranges(&self, max_range_len: u16) -> Vec<std::ops::RangeInclusive<u16>> {
        let set_bits: Vec<u16> = SetBitsIterator::new(self).collect();
        let n = set_bits.len();
        if n == 0 {
            return Vec::new();
        }
        let mut dp: Vec<(u32, u64)> = vec![(0, 0); n + 1];
        let mut choices: Vec<usize> = vec![0; n];
        for i in (0..n).rev() {
            let mut best_cost = (u32::MAX, u64::MAX);
            let mut best_choice_j = i;

            for j in i..n {
                let start_bit = set_bits[i];
                let end_bit = set_bits[j];
                let range_len = end_bit.saturating_sub(start_bit).saturating_add(1);
                if range_len > max_range_len {
                    break;
                }
                let cost_of_this_range = (1, range_len as u64);
                let cost_of_rest = dp[j + 1];
                let current_total_cost = (
                    cost_of_this_range.0 + cost_of_rest.0,
                    cost_of_this_range.1 + cost_of_rest.1,
                );
                if current_total_cost.0 < best_cost.0
                    || (current_total_cost.0 == best_cost.0 && current_total_cost.1 < best_cost.1)
                {
                    best_cost = current_total_cost;
                    best_choice_j = j;
                }
            }
            dp[i] = best_cost;
            choices[i] = best_choice_j;
        }
        let mut ranges = Vec::new();
        let mut current_bit_index = 0;
        while current_bit_index < n {
            let start_bit = set_bits[current_bit_index];
            let end_bit_index = choices[current_bit_index];
            let end_bit = set_bits[end_bit_index];
            ranges.push(start_bit..=end_bit);
            current_bit_index = end_bit_index + 1;
        }
        ranges
    }
}

pub struct SetBitsIterator<'a> {
    bitmask: &'a RegisterBitmask,
    word_index: u16,
    current_word_val: u64,
}

impl<'a> SetBitsIterator<'a> {
    pub fn new(bitmask: &'a RegisterBitmask) -> Self {
        SetBitsIterator {
            bitmask,
            word_index: 0,
            current_word_val: bitmask.0[0],
        }
    }
}

impl<'a> Iterator for SetBitsIterator<'a> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current_word_val == 0 {
                self.word_index = self.word_index.saturating_add(1);
                self.current_word_val =
                    self.bitmask.0.get(usize::from(self.word_index)).copied()?;
                continue;
            }
            let set_bit_pos = self.current_word_val.trailing_zeros() as u16;
            let address = (self.word_index * (u64::BITS as u16)) + set_bit_pos;
            self.current_word_val &= self.current_word_val - 1;
            return Some(address);
        }
    }
}

pub struct ModbusDeviceValues {
    values: [u16; u16::MAX as usize],
    have_value: RegisterBitmask,
}

impl ModbusDeviceValues {
    pub fn new() -> Self {
        Self {
            values: [0; _],
            have_value: RegisterBitmask::new(),
        }
    }

    pub fn contains(&self, address: u16) -> bool {
        self.have_value.is_set(address)
    }

    pub fn value_of(&self, register: RegisterIndex) -> Option<Value> {
        let word = self.value_of_address(register.address())?;
        Some(register.data_type().from_word(word))
    }

    pub fn value_of_address(&self, address: u16) -> Option<u16> {
        if !self.contains(address) {
            return None;
        }
        Some(self.values[address as usize])
    }

    /// Set the newly read modbus value with our cache view of the device.
    ///
    /// Returns `true` if the value has changed.
    pub fn set_value(&mut self, address: u16, value: u16) -> bool {
        let index = usize::from(address);
        let changed = value != self.values[index] || !self.have_value.is_set(address);
        self.values[index] = value;
        self.have_value.set(address);
        changed
    }

    pub fn has_all_values(&self, address_mask: &RegisterBitmask) -> bool {
        address_mask.is_subset_of(&self.have_value)
    }
}
