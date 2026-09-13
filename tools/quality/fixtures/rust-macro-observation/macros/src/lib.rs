use proc_macro::TokenStream;

#[proc_macro]
pub fn observed_function(input: TokenStream) -> TokenStream {
    match gate_observation_generator::expand(input.into()) {
        Ok(observation) => observation.generated.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
