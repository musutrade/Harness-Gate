pub fn classify(x: i32) -> i32 {
    if x > 0 && x < 10 {
        1
    } else {
        -1
    }
}

pub fn never_called() -> u32 {
    42
}

#[cfg(test)]
mod tests {
    #[test]
    fn exercise_both_sides() {
        assert_eq!(super::classify(3), 1);
        assert_eq!(super::classify(12), -1);
    }
}
