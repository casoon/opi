//! A tiny stand-in crate, so the demo's checks are real ones.

mod report;

fn main() {
    println!("pulse — {}", report::headline(42));
}

#[cfg(test)]
mod tests {
    #[test]
    fn headline_counts() {
        assert_eq!(super::report::headline(2), "2 events");
    }
}
