//! The incident count you're working towards (`src/lib/incidents.ts`). A
//! session's limit isn't the end in series that penalise there and carry
//! on: after it, the next penalty comes every `step` incidents.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// The incident count of the next penalty, None when there's no limit.
    pub next: Option<i32>,
    /// Limits already reached this session.
    pub passed: i32,
}

pub fn target(count: i32, limit: Option<i32>, step: i32) -> Target {
    let Some(limit) = limit.filter(|l| *l > 0) else { return Target { next: None, passed: 0 } };
    if count < limit {
        return Target { next: Some(limit), passed: 0 };
    }
    if step <= 0 {
        return Target { next: Some(limit), passed: 1 };
    }
    let passed = (count - limit).div_euclid(step) + 1;
    Target { next: Some(limit + passed * step), passed }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// scripts/check-incidents.ts
    #[test]
    fn check_incidents() {
        let cases: [(i32, Option<i32>, i32, Option<i32>, i32); 9] = [
            (0, None, 4, None, 0),
            (12, Some(17), 4, Some(17), 0),
            (16, Some(17), 4, Some(17), 0),
            (17, Some(17), 4, Some(21), 1),
            (20, Some(17), 4, Some(21), 1),
            (21, Some(17), 4, Some(25), 2),
            (26, Some(17), 4, Some(29), 3),
            (17, Some(17), 8, Some(25), 1),
            (19, Some(17), 0, Some(17), 1),
        ];
        for (count, limit, step, next, passed) in cases {
            assert_eq!(target(count, limit, step), Target { next, passed }, "{count}x limit {limit:?} step {step}");
        }
    }
}
