//! Syntactic decisions, deliberately distinct from compiler CFG/MIR complexity.
use anyhow::Result;
use proc_macro2::Span;
use quote::ToTokens;
use serde::Serialize;
use std::collections::BTreeSet;
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    Expr,
};

pub const SERIES: &str = "rust-source-decisions/v1-candidate";

#[derive(Debug, Serialize)]
pub struct Function {
    pub name: String,
    pub span: [usize; 4],
    pub state: &'static str,
    pub complexity: Option<u64>,
    pub coverage_eligible: bool,
    pub reasons: BTreeSet<String>,
}

#[derive(Default, Serialize)]
pub struct Inventory {
    pub functions: Vec<Function>,
    pub unsupported: BTreeSet<String>,
    pub exclusions: Vec<String>,
    pub excluded_spans: Vec<[usize; 4]>,
    #[serde(skip)]
    scope: Vec<String>,
    #[serde(skip)]
    stack: Vec<usize>,
    #[serde(skip)]
    inherited: BTreeSet<String>,
    #[serde(skip)]
    uncertified_coverage_scopes: usize,
}

fn coordinates(span: Span) -> [usize; 4] {
    let start = span.start();
    let end = span.end();
    [start.line, start.column + 1, end.line, end.column + 1]
}

fn is_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("test")
            || a.path().is_ident("cfg")
                && a.parse_args::<syn::Path>()
                    .is_ok_and(|p| p.is_ident("test"))
    })
}

fn attributes(attrs: &[syn::Attribute]) -> BTreeSet<String> {
    attrs
        .iter()
        .filter(|a| {
            ![
                "doc", "allow", "warn", "deny", "forbid", "inline", "must_use", "cold",
            ]
            .iter()
            .any(|name| a.path().is_ident(name))
        })
        .map(|a| format!("unresolved attribute: {}", a.to_token_stream()))
        .collect()
}

impl Inventory {
    fn count(&mut self, n: u64) {
        if let Some(i) = self.stack.last() {
            *self.functions[*i].complexity.as_mut().unwrap() += n;
        }
    }

    fn unsupported(&mut self, reason: &str) {
        self.unsupported.insert(reason.into());
        if let Some(i) = self.stack.last() {
            self.functions[*i].reasons.insert(reason.into());
        }
    }

    fn function(
        &mut self,
        sig: &syn::Signature,
        attrs: &[syn::Attribute],
        body: &syn::Block,
        location: Span,
    ) {
        if is_test(attrs) {
            self.excluded_spans.push(coordinates(location));
            self.exclusions
                .push(format!("test function: {}", sig.ident));
            return;
        }
        let mut reasons = self.inherited.clone();
        reasons.extend(attributes(attrs));
        if sig.asyncness.is_some() {
            reasons.insert("async lowering owner not certified".into());
        }
        if !sig.generics.params.is_empty() {
            reasons.insert("generic instantiation owner not certified".into());
        }
        if !self.stack.is_empty() {
            self.unsupported("nested function owner not certified");
            reasons.insert("nested function owner not certified".into());
        }
        let start = location.start();
        let end = location.end();
        let mut scope = self.scope.clone();
        scope.push(sig.ident.to_string());
        self.stack.push(self.functions.len());
        self.functions.push(Function {
            name: scope.join("::"),
            span: [start.line, start.column + 1, end.line, end.column + 1],
            state: "supported",
            complexity: Some(1),
            coverage_eligible: self.uncertified_coverage_scopes == 0
                && attrs.is_empty()
                && reasons.is_empty(),
            reasons,
        });
        self.visit_signature(sig);
        self.visit_block(body);
        self.stack.pop();
    }
}

impl<'ast> Visit<'ast> for Inventory {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if is_test(&node.attrs) {
            self.excluded_spans.push(coordinates(node.span()));
            self.exclusions.push(format!("test module: {}", node.ident));
            return;
        }
        let previous = self.inherited.clone();
        self.inherited.extend(attributes(&node.attrs));
        if node.content.is_none() {
            self.unsupported("external module activation/ownership not certified");
        }
        let uncertified = usize::from(!node.attrs.is_empty() || node.content.is_none());
        self.uncertified_coverage_scopes += uncertified;
        self.scope.push(node.ident.to_string());
        visit::visit_item_mod(self, node);
        self.scope.pop();
        self.uncertified_coverage_scopes -= uncertified;
        self.inherited = previous;
    }
    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let previous = self.inherited.clone();
        self.inherited.extend(attributes(&node.attrs));
        if !node.generics.params.is_empty() || node.trait_.is_some() {
            self.inherited
                .insert("generic or trait impl owner not certified".into());
        }
        self.scope.push(node.self_ty.to_token_stream().to_string());
        self.uncertified_coverage_scopes += 1;
        visit::visit_item_impl(self, node);
        self.uncertified_coverage_scopes -= 1;
        self.scope.pop();
        self.inherited = previous;
    }
    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        let previous = self.inherited.clone();
        self.inherited
            .insert("trait default owner not certified".into());
        self.scope.push(node.ident.to_string());
        self.uncertified_coverage_scopes += 1;
        visit::visit_item_trait(self, node);
        self.uncertified_coverage_scopes -= 1;
        self.scope.pop();
        self.inherited = previous;
    }
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.function(&node.sig, &node.attrs, &node.block, node.span());
    }
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.function(&node.sig, &node.attrs, &node.block, node.span());
    }
    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if let Some(body) = &node.default {
            self.function(&node.sig, &node.attrs, body, node.span());
        }
    }
    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        for reason in attributes(std::slice::from_ref(node)) {
            self.unsupported(&reason);
        }
    }
    fn visit_type_impl_trait(&mut self, _: &'ast syn::TypeImplTrait) {
        self.unsupported("opaque or argument-position impl Trait owner not certified");
    }
    fn visit_macro(&mut self, _: &'ast syn::Macro) {
        self.unsupported("macro expansion / generated owner not certified");
    }
    fn visit_expr(&mut self, node: &'ast Expr) {
        match node {
            Expr::Closure(_) => {
                self.unsupported("closure owner not certified; no parent coverage inheritance");
                return;
            }
            Expr::Async(_) => self.unsupported("async block owner not certified"),
            Expr::If(_) | Expr::While(_) | Expr::ForLoop(_) | Expr::Loop(_) | Expr::Try(_) => {
                self.count(1)
            }
            Expr::Binary(b) if matches!(b.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) => {
                self.count(1)
            }
            Expr::Match(m) => self.count(
                m.arms.len().saturating_sub(1) as u64
                    + m.arms.iter().filter(|a| a.guard.is_some()).count() as u64,
            ),
            _ => (),
        }
        visit::visit_expr(self, node);
    }
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if node.init.as_ref().is_some_and(|i| i.diverge.is_some()) {
            self.count(1);
        }
        visit::visit_local(self, node);
    }
}

pub fn analyze(source: &str) -> Result<Inventory> {
    let file = syn::parse_file(source)?;
    let mut inventory = Inventory::default();
    inventory.inherited.extend(attributes(&file.attrs));
    inventory.visit_file(&file);
    for function in &mut inventory.functions {
        if !function.reasons.is_empty() {
            function.state = "unsupported";
            function.coverage_eligible = false;
            function.complexity = None;
        }
    }
    Ok(inventory)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_modules_preserve_free_function_owners_and_scope_boundaries() {
        let inventory = analyze("mod left { fn f() {} mod nested { fn unused() {} } } mod right { fn f() {} } #[allow(dead_code)] mod annotated { fn f() {} } struct S; impl S { fn f() {} } fn root() {}").unwrap();
        assert_eq!(
            inventory
                .functions
                .iter()
                .map(|f| (f.name.as_str(), f.coverage_eligible))
                .collect::<Vec<_>>(),
            [
                ("left::f", true),
                ("left::nested::unused", true),
                ("right::f", true),
                ("annotated::f", false),
                ("S::f", false),
                ("root", true)
            ]
        );
        for text in [
            "#[cfg(feature=\"x\")] mod m { fn f() {} }",
            "fn outer() { mod m { fn inner() {} } }",
        ] {
            assert!(analyze(text)
                .unwrap()
                .functions
                .iter()
                .all(|f| !f.coverage_eligible));
        }
    }
    #[test]
    fn explicit_decisions_and_unexecuted_source_functions() {
        let inventory = analyze("fn branch(x: bool) { if x && x { } else { } } fn unused() {} #[cfg(test)] mod tests { #[test] fn t() { panic!() } }").unwrap();
        assert_eq!(inventory.functions.len(), 2);
        assert_eq!(inventory.functions[0].complexity, Some(3));
        assert_eq!(inventory.functions[1].complexity, Some(1));
        assert_eq!(inventory.exclusions.len(), 1);
    }
    #[test]
    fn unsupported_syntax_never_becomes_a_number() {
        for source in [
            "async fn f() {}",
            "fn f<T>() {}",
            "fn f(x: impl Copy) {}",
            "fn f() { let _ = || 1; }",
            "fn f() { panic!(); }",
            "#[cfg(feature=\"x\")] fn f() {}",
            "fn f(){ #[cfg(feature=\"x\")] let x = 2; }",
            "#[cfg(feature=\"x\")] mod m { fn f() {} }",
            "impl<T> A<T> { fn f() {} }",
        ] {
            let result = analyze(source).unwrap();
            assert!(!result.functions.is_empty(), "{source}");
            assert!(
                result
                    .functions
                    .iter()
                    .all(|f| f.state == "unsupported" && f.complexity.is_none()),
                "{source}"
            );
        }
        assert!(
            !analyze("#[derive(Debug)] struct X; include!(\"generated.rs\");")
                .unwrap()
                .unsupported
                .is_empty()
        );
        assert!(analyze("fn broken(").is_err());
    }
}
