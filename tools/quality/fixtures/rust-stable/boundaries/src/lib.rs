#[derive(Debug, Clone)]
pub struct Derived(pub u8);

macro_rules! make_function {
    ($name:ident) => { pub fn $name() -> u8 { 7 } };
}
make_function!(expanded);

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

pub async fn deferred() -> u8 { 3 }
pub fn generic<T>(x: T) -> T { x }
pub fn closure(x: bool) -> u8 {
    let f = || if x { 1 } else { 2 };
    f()
}

#[cfg(feature = "extra")]
pub fn feature_only() -> u8 { 4 }

pub fn never_called() -> u8 { 5 }

#[cfg(test)]
mod tests {
    #[test]
    fn exercise_expanded_and_distinct_owners() {
        assert_eq!(super::expanded(), 7);
        assert_eq!(super::generated(true), 1);
        assert_eq!(super::closure(false), 2);
        assert_eq!(super::generic(1u8), 1);
        let _future = super::deferred();
        let _derived = format!("{:?}", super::Derived(1).clone());
        #[cfg(feature = "extra")]
        assert_eq!(super::feature_only(), 4);
    }
}
