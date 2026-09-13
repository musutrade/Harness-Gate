pub mod left {
    pub fn classify(x: bool) -> u32 {
        if x {
            1
        } else {
            2
        }
    }

    pub mod nested {
        pub fn never_called() -> u32 {
            42
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn one_side() {
            assert_eq!(super::classify(true), 1);
        }
    }
}

pub mod right {
    pub fn classify(x: bool) -> u32 {
        if x {
            3
        } else {
            4
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn both_sides() {
            assert_eq!(super::classify(true), 3);
            assert_eq!(super::classify(false), 4);
        }
    }
}
