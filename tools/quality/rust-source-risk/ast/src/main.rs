use proc_macro2::Span;
use serde::Serialize;
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
};

#[derive(Serialize)]
struct Callable {
    name: String,
    kind: &'static str,
    start: [usize; 2],
    end: [usize; 2],
    anchors: Vec<[usize; 2]>,
    body: [usize; 2],
    asynchronous: bool,
    complexity: u64,
}
struct Inventory<'a> {
    source: &'a str,
    functions: Vec<Callable>,
    stack: Vec<usize>,
    errors: Vec<String>,
}
impl Inventory<'_> {
    fn point(&self, p: proc_macro2::LineColumn) -> [usize; 2] {
        let line = self.source.lines().nth(p.line - 1).unwrap_or("");
        [
            p.line,
            line.chars()
                .take(p.column)
                .map(char::len_utf8)
                .sum::<usize>()
                + 1,
        ]
    }
    fn begin(
        &mut self,
        name: String,
        kind: &'static str,
        span: Span,
        anchors: &[Span],
        body: Span,
        asynchronous: bool,
    ) {
        let index = self.functions.len();
        self.functions.push(Callable {
            name,
            kind,
            start: self.point(span.start()),
            end: self.point(span.end()),
            anchors: anchors.iter().map(|s| self.point(s.start())).collect(),
            body: self.point(body.start()),
            asynchronous,
            complexity: 1,
        });
        self.stack.push(index);
    }
    fn decision(&mut self, n: usize) {
        if let Some(i) = self.stack.last() {
            self.functions[*i].complexity += n as u64;
        }
    }
    fn unsupported(&mut self, kind: &str, span: Span) {
        self.errors
            .push(format!("{kind} at {:?}", self.point(span.start())));
    }
}
impl<'ast> Visit<'ast> for Inventory<'_> {
    fn visit_attribute(&mut self, n: &'ast syn::Attribute) {
        let path = n
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        if ![
            "doc",
            "derive",
            "repr",
            "inline",
            "cold",
            "must_use",
            "allow",
            "warn",
            "deny",
            "forbid",
            "deprecated",
            "tokio::main",
        ]
        .contains(&path.as_str())
        {
            self.unsupported(&format!("unsupported attribute {path}"), n.span());
        }
    }
    fn visit_item_fn(&mut self, n: &'ast syn::ItemFn) {
        for attr in &n.attrs {
            self.visit_attribute(attr);
        }
        self.begin(
            n.sig.ident.to_string(),
            "function",
            n.span(),
            &[
                if matches!(n.vis, syn::Visibility::Inherited) {
                    n.sig.span()
                } else {
                    n.vis.span()
                },
                n.sig.span(),
                n.sig.fn_token.span,
                n.block.brace_token.span.open(),
            ],
            n.block.brace_token.span.open(),
            n.sig.asyncness.is_some(),
        );
        visit::visit_block(self, &n.block);
        self.stack.pop();
    }
    fn visit_impl_item_fn(&mut self, n: &'ast syn::ImplItemFn) {
        for attr in &n.attrs {
            self.visit_attribute(attr);
        }
        self.begin(
            n.sig.ident.to_string(),
            "method",
            n.span(),
            &[
                if matches!(n.vis, syn::Visibility::Inherited) {
                    n.sig.span()
                } else {
                    n.vis.span()
                },
                n.sig.span(),
                n.sig.fn_token.span,
                n.block.brace_token.span.open(),
            ],
            n.block.brace_token.span.open(),
            n.sig.asyncness.is_some(),
        );
        visit::visit_block(self, &n.block);
        self.stack.pop();
    }
    fn visit_trait_item_fn(&mut self, n: &'ast syn::TraitItemFn) {
        for attr in &n.attrs {
            self.visit_attribute(attr);
        }
        if let Some(block) = &n.default {
            self.begin(
                n.sig.ident.to_string(),
                "default-method",
                n.span(),
                &[
                    n.sig.span(),
                    n.sig.fn_token.span,
                    block.brace_token.span.open(),
                ],
                block.brace_token.span.open(),
                n.sig.asyncness.is_some(),
            );
            visit::visit_block(self, block);
            self.stack.pop();
        }
    }
    fn visit_expr_closure(&mut self, n: &'ast syn::ExprClosure) {
        for attr in &n.attrs {
            self.visit_attribute(attr);
        }
        self.begin(
            "<closure>".into(),
            "closure",
            n.span(),
            &[n.span(), n.body.span()],
            n.body.span(),
            n.asyncness.is_some(),
        );
        self.visit_expr(&n.body);
        self.stack.pop();
    }
    fn visit_expr_async(&mut self, n: &'ast syn::ExprAsync) {
        for attr in &n.attrs {
            self.visit_attribute(attr);
        }
        self.begin(
            "<async>".into(),
            "async-block",
            n.span(),
            &[n.span(), n.block.brace_token.span.open()],
            n.block.brace_token.span.open(),
            true,
        );
        visit::visit_block(self, &n.block);
        self.stack.pop();
    }
    fn visit_expr_if(&mut self, n: &'ast syn::ExprIf) {
        self.decision(1);
        visit::visit_expr_if(self, n);
    }
    fn visit_expr_for_loop(&mut self, n: &'ast syn::ExprForLoop) {
        self.decision(1);
        visit::visit_expr_for_loop(self, n);
    }
    fn visit_expr_while(&mut self, n: &'ast syn::ExprWhile) {
        self.decision(1);
        visit::visit_expr_while(self, n);
    }
    fn visit_expr_match(&mut self, n: &'ast syn::ExprMatch) {
        self.decision(
            n.arms.len().saturating_sub(1) + n.arms.iter().filter(|a| a.guard.is_some()).count(),
        );
        visit::visit_expr_match(self, n);
    }
    fn visit_expr_binary(&mut self, n: &'ast syn::ExprBinary) {
        if matches!(n.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
            self.decision(1);
        }
        visit::visit_expr_binary(self, n);
    }
    fn visit_expr_try(&mut self, n: &'ast syn::ExprTry) {
        self.decision(1);
        visit::visit_expr_try(self, n);
    }
    fn visit_local(&mut self, n: &'ast syn::Local) {
        if n.init.as_ref().is_some_and(|i| i.diverge.is_some()) {
            self.decision(1);
        }
        visit::visit_local(self, n);
    }
    fn visit_item_macro(&mut self, n: &'ast syn::ItemMacro) {
        self.unsupported("item macro", n.span());
    }
    fn visit_macro(&mut self, n: &'ast syn::Macro) {
        use syn::parse::Parser;
        let path = n
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        if path == "matches" {
            let parser =
                |input: syn::parse::ParseStream| -> syn::Result<(syn::Expr, Option<syn::Expr>)> {
                    let expression = input.parse()?;
                    input.parse::<syn::Token![,]>()?;
                    syn::Pat::parse_multi_with_leading_vert(input)?;
                    let guard = if input.peek(syn::Token![if]) {
                        input.parse::<syn::Token![if]>()?;
                        Some(input.parse()?)
                    } else {
                        None
                    };
                    if input.peek(syn::Token![,]) {
                        input.parse::<syn::Token![,]>()?;
                    }
                    Ok((expression, guard))
                };
            match parser.parse2(n.tokens.clone()) {
                Ok((expression, guard)) => {
                    self.decision(1 + usize::from(guard.is_some()));
                    self.visit_expr(&expression);
                    if let Some(g) = guard {
                        self.visit_expr(&g);
                    }
                }
                Err(_) => self.unsupported("unparsed matches!", n.span()),
            }
        } else if [
            "tracing::info",
            "tracing::warn",
            "tracing::error",
            "tracing::debug",
            "tracing::trace",
            "sqlx::migrate",
        ]
        .contains(&path.as_str())
        {
            let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
            match parser.parse2(n.tokens.clone()) {
                Ok(expressions) => {
                    for e in expressions {
                        self.visit_expr(&e);
                    }
                }
                Err(_) => self.unsupported("unsupported macro arguments", n.span()),
            }
        } else {
            self.unsupported(&format!("unrecognized macro {path}!"), n.span());
        }
    }
}
fn inventory(source: &str) -> Result<Vec<Callable>, String> {
    let syntax = syn::parse_file(source).map_err(|e| e.to_string())?;
    let mut visitor = Inventory {
        source,
        functions: vec![],
        stack: vec![],
        errors: vec![],
    };
    visitor.visit_file(&syntax);
    if !visitor.errors.is_empty() {
        return Err(visitor.errors.join("; "));
    }
    Ok(visitor.functions)
}
fn main() {
    let result = (|| -> Result<(), String> {
        let path = std::env::args().nth(1).ok_or("expected Rust source path")?;
        let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string(&inventory(&source)?).map_err(|e| e.to_string())?
        );
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_decisions() {
        let f = inventory("fn f(x: Option<i32>) -> Option<i32> { let Some(y) = x else { return None }; if y > 0 && y < 4 { for _ in 0..y {} } while false {} match y { 0 => {}, 1 if true => {}, _ => {} } Some(x?) }").unwrap();
        assert_eq!(f[0].complexity, 10);
    }
    #[test]
    fn nested_owners() {
        let f = inventory(
            "async fn f() { let g = || if true { 1 } else { 2 }; let h = async { if false {} }; }",
        )
        .unwrap();
        assert_eq!(
            f.iter().map(|f| f.complexity).collect::<Vec<_>>(),
            vec![1, 2, 2]
        );
    }
    #[test]
    fn macros_count_only_source() {
        let f = inventory(
            "fn f(x: Option<i32>) { tracing::info!(\"{}\", x?); matches!(x, Some(y) if y > 0); }",
        )
        .unwrap();
        assert_eq!(f[0].complexity, 4);
        assert!(inventory("fn f() { custom!(); }").is_err());
        assert!(inventory("macro_rules! x { () => {} }").is_err());
        assert!(inventory("#[cfg(feature = \"x\")] fn f() {}").is_err());
    }
    #[test]
    fn unicode_columns_are_bytes() {
        let src = "fn f() { let café = 1; let g = || café; }";
        let f = inventory(src).unwrap();
        assert_eq!(f[1].start[1], src.find("||").unwrap() + 1);
    }
    #[test]
    fn async_and_methods() {
        let f = inventory(
            "struct A; impl A { async fn run(&self) { if true {} } } trait T { fn f() {} }",
        )
        .unwrap();
        assert_eq!(f.len(), 2);
        assert!(f[0].asynchronous);
        assert_eq!(f[0].complexity, 2);
    }
}
