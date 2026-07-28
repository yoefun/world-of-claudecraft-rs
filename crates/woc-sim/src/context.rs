//! Thin SimContext seam (emit + lookups). Framework stub for future leaf extraction.

use woc_protocol::SimEvent;

/// Callback bag held by `Sim` during a tick. Leaf modules can take `&mut SimContext`
/// without depending on the full `Sim` facade.
pub struct SimContext<'a> {
    pub events: &'a mut Vec<SimEvent>,
}

impl<'a> SimContext<'a> {
    pub fn emit(&mut self, event: SimEvent) {
        self.events.push(event);
    }
}
