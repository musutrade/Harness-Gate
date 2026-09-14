//! The generated functions are real source in a real file via include!, not a
//! proc-macro token stream. That is what lets stable coverage own each function.
include!(concat!(env!("OUT_DIR"), "/generated_owners.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_functions_execute_real_code() {
        assert_eq!(plain(-3), -3);
        assert_eq!(branch(2), 1);
        assert_eq!(branch(-2), 0);
        // Deliberately never execute `unexecuted`; coverage must show a real zero.
    }
    #[test]
    fn configuration_selects_the_actual_invocation() {
        assert_eq!(
            configured(7),
            if cfg!(feature = "branching") { 1 } else { 7 }
        );
    }
}
