#[derive(Clone, Copy, Default, Debug)]
pub struct Sample {
    pub y: f64,
    pub y_rot: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DragonFlightHistory {
    samples: [Sample; 64],
    head: i32,
}

impl Default for DragonFlightHistory {
    fn default() -> Self {
        Self {
            samples: [Sample::default(); 64],
            head: -1,
        }
    }
}

impl DragonFlightHistory {
    pub const fn record(&mut self, y: f64, y_rot: f32) {
        self.head += 1;
        if self.head >= 64 {
            self.head = 0;
        }
        self.samples[self.head as usize] = Sample { y, y_rot };
    }

    #[must_use]
    pub fn get(&self, offset: i32) -> Sample {
        if self.head < 0 {
            return Sample::default();
        }
        let mut index = (self.head - offset) & 63;
        if index < 0 {
            index += 64;
        }
        self.samples[index as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragon_flight_history_ring_buffer_wrapping() {
        let mut history = DragonFlightHistory::default();
        assert_eq!(history.get(0).y, 0.0);

        history.record(100.0, 45.0);
        assert_eq!(history.get(0).y, 100.0);
        assert_eq!(history.get(0).y_rot, 45.0);

        // Fill past 64 to test circular wrap
        for i in 1..=70 {
            history.record(100.0 + f64::from(i), 45.0);
        }
        assert_eq!(history.get(0).y, 170.0);
        assert_eq!(history.get(1).y, 169.0);
    }
}
