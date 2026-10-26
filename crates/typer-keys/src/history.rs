//! The short list of recent keys the overlay shows. Pure logic, no drawing.

use std::time::{Duration, Instant};

/// Same key again within this window counts up ("A ×3") instead of adding an entry.
const REPEAT_WINDOW: Duration = Duration::from_millis(1000);
/// How long an entry stays fully visible after its last press.
const HOLD: Duration = Duration::from_millis(1800);
const FADE: Duration = Duration::from_millis(250);
const MAX_ITEMS: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub label: String,
    pub count: u32,
    pub last: Instant,
}

impl Item {
    pub fn text(&self) -> String {
        if self.count > 1 {
            format!("{} ×{}", self.label, self.count)
        } else {
            self.label.clone()
        }
    }

    /// 1.0 while held, fading to 0.0.
    pub fn alpha(&self, now: Instant) -> f64 {
        let age = now.saturating_duration_since(self.last);
        if age <= HOLD {
            return 1.0;
        }
        1.0 - ((age - HOLD).as_secs_f64() / FADE.as_secs_f64()).min(1.0)
    }
}

#[derive(Debug, Default)]
pub struct History {
    pub items: Vec<Item>,
}

impl History {
    pub fn push(&mut self, label: String, now: Instant) {
        if let Some(last) = self.items.last_mut()
            && last.label == label
            && now.saturating_duration_since(last.last) < REPEAT_WINDOW
        {
            last.count += 1;
            last.last = now;
            return;
        }
        self.items.push(Item {
            label,
            count: 1,
            last: now,
        });
        if self.items.len() > MAX_ITEMS {
            self.items.remove(0);
        }
    }

    /// Drop fully faded entries; true while anything is still visible.
    pub fn prune(&mut self, now: Instant) -> bool {
        self.items.retain(|i| i.alpha(now) > 0.0);
        !self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_count_up() {
        let t = Instant::now();
        let mut h = History::default();
        for i in 0..3 {
            h.push("A".into(), t + Duration::from_millis(100 * i));
        }
        assert_eq!(h.items.len(), 1);
        assert_eq!(h.items[0].text(), "A ×3");
        h.push("A".into(), t + Duration::from_millis(1500));
        assert_eq!(h.items.len(), 2, "a late repeat starts a new entry");
    }

    #[test]
    fn caps_and_fades() {
        let t = Instant::now();
        let mut h = History::default();
        for c in 'A'..='J' {
            h.push(c.to_string(), t);
        }
        assert_eq!(h.items.len(), MAX_ITEMS);
        assert_eq!(h.items[0].label, "C");
        assert_eq!(h.items[0].alpha(t + HOLD), 1.0);
        assert!(h.items[0].alpha(t + HOLD + FADE / 2) < 1.0);
        assert!(!h.prune(t + HOLD + FADE));
    }
}
