use std::collections::HashMap;

use crate::emulator::Emulator;

// number of instructions
const B_STATE_INTERVAL: u64 = 10000;
const B_STATE_LIMIT: usize = 250;

pub struct TimeTravel {
    pub current: Emulator,
    history: HashMap<u64, Emulator>,
    smallest_b_state: u64,
}

impl TimeTravel {
    pub fn new(emulator: Emulator) -> TimeTravel {
        let mut history = HashMap::default();
        history.insert(0, emulator.clone());

        TimeTravel {
            current: emulator.clone(),
            history,
            smallest_b_state: 0,
        }
    }

    pub fn step(&mut self, amount: i32) -> Option<u64> {
        if amount >= 0 {
            for _ in 0..amount {
                match self.current.fetch_and_execute() {
                    Ok(Some(exit_code)) => return Some(exit_code),
                    Ok(None) => {}
                    Err(e) => {
                        self.current.stderr.push_str(&e.to_string());
                        return None;
                    }
                }

                let i = self.current.inst_counter / B_STATE_INTERVAL;
                let r = self.current.inst_counter % B_STATE_INTERVAL;

                // only add if greater than current latest timestamp
                if i >= self.history.len() as u64 && r == 0 {
                    self.history.insert(i, self.current.clone());

                    if self.history.len() > B_STATE_LIMIT {
                        assert!(self.history.remove(&self.smallest_b_state).is_some());
                        self.smallest_b_state += 1;
                    }
                }

                debug_assert!(self.history.len() <= B_STATE_LIMIT);
            }
        } else {
            // find closest one
            let new_inst_count = self.current.inst_counter as i64 + amount as i64;
            if new_inst_count < 0 {
                return None;
            }

            let i = new_inst_count as u64 / B_STATE_INTERVAL;
            let r = new_inst_count as u64 % B_STATE_INTERVAL;

            match self.history.get(&i) {
                Some(new_current) => {
                    self.current = new_current.clone();

                    for _ in 0..r {
                        // guaranteed to not return
                        match self.current.fetch_and_execute() {
                            Ok(Some(exit_code)) => return Some(exit_code),
                            Ok(None) => {}
                            Err(e) => {
                                self.current.stderr.push_str(&e.to_string());
                                return None;
                            }
                        }
                    }
                }
                None => {
                    self.current = self.history[&self.smallest_b_state].clone();
                }
            }
        }

        None
    }
}
