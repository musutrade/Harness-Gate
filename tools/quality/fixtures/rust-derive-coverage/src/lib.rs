#[derive(Clone)]
pub struct One(pub u8);
#[derive(Clone)]
pub struct Two(pub u8, pub u8);
#[derive(Clone)]
pub struct Unused(pub u8);
#[derive(Clone)]
pub struct Configured {
    pub value: u8,
    #[cfg(feature = "extra")]
    pub extra: u8,
}

pub fn clone_one(value: &One) -> u8 {
    value.clone().0
}
pub fn clone_two(value: &Two) -> u8 {
    let cloned = value.clone();
    cloned.0 + cloned.1
}
pub fn clone_unused(value: &Unused) -> u8 {
    value.clone().0
}
pub fn clone_configured(value: &Configured) -> u8 {
    let cloned = value.clone();
    #[cfg(feature = "extra")]
    {
        cloned.value + cloned.extra
    }
    #[cfg(not(feature = "extra"))]
    {
        cloned.value
    }
}

// Diagnostic controls isolate the compiler's automatically_derived filter.
// Neither implementation is a workaround used by the collector.
pub struct Manual(pub u8);
impl Clone for Manual {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
pub struct Annotated(pub u8);
#[automatically_derived]
impl Clone for Annotated {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn annotation_control_executes() {
        assert_eq!(Manual(11).clone().0, 11);
        assert_eq!(Annotated(13).clone().0, 13);
    }
    #[test]
    fn distinct_derive_inputs_execute() {
        assert_eq!(clone_one(&One(3)), 3);
        assert_eq!(clone_two(&Two(3, 5)), 8);
    }
    #[test]
    fn configured_derive_executes() {
        let value = Configured {
            value: 7,
            #[cfg(feature = "extra")]
            extra: 2,
        };
        assert_eq!(
            clone_configured(&value),
            if cfg!(feature = "extra") { 9 } else { 7 }
        );
    }
}
