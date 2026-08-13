#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gold {
    pub copper: u32,
}

impl Gold {
    pub fn try_spend(&mut self, amount: u32) -> bool {
        if self.copper < amount {
            return false;
        }
        self.copper -= amount;
        true
    }
}
