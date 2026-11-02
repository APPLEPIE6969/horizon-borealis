//! Image comparison for the golden tests: the mean and tail of the absolute
//! per-channel difference between a render and its committed reference.

#[derive(Debug, Clone, Copy)]
pub struct Diff {
    /// Mean absolute difference per channel, 0..255.
    pub mean: f32,
    /// The per-channel absolute difference that 99% of channels stay under.
    pub p99: f32,
    /// The largest absolute difference on any channel.
    pub max: f32,
    /// The share of channels that differ by more than 8/255.
    pub bad_share: f32,
}

pub fn diff(a: &[u8], b: &[u8]) -> Diff {
    assert_eq!(a.len(), b.len(), "images differ in size");
    let mut total = 0u64;
    let mut bad = 0u64;
    let mut max = 0u8;
    let mut hist = [0u64; 256];
    for (x, y) in a.iter().zip(b.iter()) {
        let d = x.abs_diff(*y);
        total += d as u64;
        hist[d as usize] += 1;
        max = max.max(d);
        if d > 8 {
            bad += 1;
        }
    }
    let n = a.len() as f32;
    let mut acc = 0u64;
    let mut p99 = 255u8;
    for (v, count) in hist.iter().enumerate() {
        acc += count;
        if acc as f32 >= n * 0.99 {
            p99 = v as u8;
            break;
        }
    }
    Diff {
        mean: total as f32 / n,
        p99: p99 as f32,
        max: max as f32,
        bad_share: bad as f32 / n,
    }
}

impl Diff {
    /// The golden test's tolerances: SwiftShader and a real GPU differ by a
    /// rounding here and there, not by a look.
    pub fn within_golden_tolerance(&self) -> bool {
        self.mean <= 2.0 && self.p99 <= 12.0 && self.bad_share <= 0.02
    }
}
