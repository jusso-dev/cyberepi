/// Binary tree of non-negative event rates.
///
/// `find` returns the leaf whose prefix sum first exceeds the target. It is the
/// selection step of the Gillespie direct method.
#[derive(Clone, Debug)]
pub struct RateTree {
    /// Power-of-two leaf capacity.
    n: usize,
    size: usize,
    t: Vec<f64>,
}

impl RateTree {
    pub fn new(size: usize) -> Self {
        let n = size.max(1).next_power_of_two();
        Self {
            n,
            size,
            t: vec![0.0; n * 2],
        }
    }

    pub fn set(&mut self, index: usize, value: f64) {
        debug_assert!(index < self.size);
        let value = if value.is_finite() && value > 0.0 {
            value
        } else {
            0.0
        };
        let mut cursor = self.n + index;
        self.t[cursor] = value;
        while cursor > 1 {
            cursor >>= 1;
            self.t[cursor] = self.t[cursor << 1] + self.t[(cursor << 1) | 1];
        }
    }

    pub fn total(&self) -> f64 {
        self.t[1]
    }

    pub fn find(&self, mut value: f64) -> usize {
        if self.size == 0 || self.t[1] <= 0.0 {
            return 0;
        }
        if !value.is_finite() || value < 0.0 {
            value = 0.0;
        }
        if value >= self.t[1] {
            value = self.t[1] * (1.0 - f64::EPSILON * 8.0);
        }
        let mut cursor = 1usize;
        while cursor < self.n {
            let left = cursor << 1;
            let left_sum = self.t[left];
            if value < left_sum {
                cursor = left;
            } else {
                value -= left_sum;
                cursor = left | 1;
            }
        }
        let leaf = cursor - self.n;
        if leaf >= self.size {
            self.size - 1
        } else {
            leaf
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_leaves_by_weight() {
        let mut tree = RateTree::new(4);
        tree.set(0, 0.0);
        tree.set(1, 5.0);
        tree.set(2, 0.0);
        tree.set(3, 5.0);
        assert!((tree.total() - 10.0).abs() < 1e-9);
        assert_eq!(tree.find(0.0), 1);
        assert_eq!(tree.find(4.9), 1);
        assert_eq!(tree.find(5.0), 3);
        assert_eq!(tree.find(9.9), 3);
    }

    #[test]
    fn update_replaces_weight() {
        let mut tree = RateTree::new(3);
        tree.set(0, 2.0);
        tree.set(1, 2.0);
        tree.set(0, 0.0);
        assert!((tree.total() - 2.0).abs() < 1e-9);
        assert_eq!(tree.find(0.1), 1);
    }
}
