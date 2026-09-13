pub fn decimal_length(value: u32) -> usize {
    itoa::Buffer::new().format(value).len()
}

#[cfg(test)]
mod tests {
    #[test]
    fn registry_dependency_executes() {
        assert_eq!(super::decimal_length(1234), 4);
    }
}
