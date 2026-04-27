use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Expr, ExprCall, ExprMethodCall, ExprParen, ExprPath, ExprReference, Lit};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Diagnostic {
    file: PathBuf,
    line: usize,
    column: usize,
    rule: &'static str,
    message: String,
}

struct LayoutLintVisitor<'a> {
    file: &'a Path,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for LayoutLintVisitor<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let chain = ChainInfo::from_expr(&Expr::MethodCall(node.clone()));
        self.check_requires_layout_container(&chain);
        self.check_prefer_layout_helpers(&chain);
        self.check_prefer_design_system_helpers(&chain);

        for arg in &node.args {
            self.visit_expr(arg);
        }
    }
}

impl LayoutLintVisitor<'_> {
    fn in_views_dir(&self) -> bool {
        self.file
            .components()
            .any(|component| component.as_os_str() == "views")
    }

    fn check_requires_layout_container(&mut self, chain: &ChainInfo) {
        if chain.has_layout_container() {
            return;
        }

        let Some(trigger) = chain
            .methods
            .iter()
            .find(|method| method_requires_layout_container(method.name.as_str()))
        else {
            return;
        };

        self.push(
            trigger.span,
            "missing-layout-container",
            format!(
                "`{}` needs a flex or grid container. Add `.flex()`/`.grid()` or switch to `.h_flex()` / `.v_flex()`.",
                trigger.name
            ),
        );
    }

    fn check_prefer_design_system_helpers(&mut self, chain: &ChainInfo) {
        if !self.in_views_dir() {
            return;
        }

        self.check_prefer_page_stack(chain);
        self.check_prefer_section_stack(chain);
        self.check_prefer_semantic_spacing_tokens(chain);
    }

    fn check_prefer_page_stack(&mut self, chain: &ChainInfo) {
        if chain.uses_design_system_helper() || chain.root_name.as_deref() != Some("v_flex") {
            return;
        }

        let Some(gap) = chain.method("gap") else {
            return;
        };

        if chain.child_like_count() < 2 {
            return;
        }

        if !is_page_spacing_expr(gap.args.first()) {
            return;
        }

        self.push(
            gap.span,
            "prefer-page-stack",
            "Prefer `page_stack()` over a hand-written `v_flex().gap(...)` page wrapper in `src/views`.".to_string(),
        );
    }

    fn check_prefer_section_stack(&mut self, chain: &ChainInfo) {
        if chain.uses_design_system_helper() || chain.root_name.as_deref() != Some("panel") {
            return;
        }

        let Some(padding) = chain.method("p") else {
            return;
        };
        let Some(gap) = chain.method("gap") else {
            return;
        };

        if chain.child_like_count() < 1 {
            return;
        }

        if !is_section_padding_expr(padding.args.first()) || !is_section_gap_expr(gap.args.first())
        {
            return;
        }

        self.push(
            padding.span,
            "prefer-section-stack",
            "Prefer `section_stack(level)` over a hand-written `panel(...).p(...).v_flex().gap(...)` section wrapper in `src/views`.".to_string(),
        );
    }

    fn check_prefer_semantic_spacing_tokens(&mut self, chain: &ChainInfo) {
        if chain.uses_design_system_helper() || !chain.is_obvious_view_container() {
            return;
        }

        let Some(trigger) = chain.methods.iter().find(|method| {
            matches!(method.name.as_str(), "gap" | "p" | "px" | "py")
                && is_raw_spacing_expr(method.args.first())
                && chain.allows_spacing_method(method.name.as_str())
        }) else {
            return;
        };

        self.push(
            trigger.span,
            "prefer-semantic-spacing-token",
            "Prefer semantic spacing tokens such as `heading_gap()`, `inline_gap()`, `section_gap()`, `page_gap()`, `panel_padding()`, or `page_padding()` instead of raw `space_*` / `px(4|8|12)` values in obvious view containers.".to_string(),
        );
    }

    fn check_prefer_layout_helpers(&mut self, chain: &ChainInfo) {
        if self.file.ends_with("src/components/layout.rs") || chain.uses_layout_helper() {
            return;
        }

        if chain.has_method("flex") && chain.has_method("flex_col") {
            let span = chain
                .method("flex_col")
                .map(|method| method.span)
                .unwrap_or(chain.span);
            self.push(
                span,
                "prefer-v-flex",
                "Prefer `.v_flex()` over `.flex().flex_col()` so flex display and direction stay coupled.".to_string(),
            );
        }

        if chain.has_method("flex")
            && chain.has_method("flex_row")
            && chain.has_method("items_center")
        {
            let span = chain
                .method("flex_row")
                .map(|method| method.span)
                .unwrap_or(chain.span);
            self.push(
                span,
                "prefer-h-flex",
                "Prefer `.h_flex()` over `.flex().flex_row().items_center()` so the common row layout stays consistent.".to_string(),
            );
        }
    }

    fn push(&mut self, span: Span, rule: &'static str, message: String) {
        let start = span.start();
        self.diagnostics.push(Diagnostic {
            file: self.file.to_path_buf(),
            line: start.line,
            column: start.column + 1,
            rule,
            message,
        });
    }
}

#[derive(Clone)]
struct MethodInfo {
    name: String,
    span: Span,
    args: Vec<Expr>,
}

#[derive(Clone)]
struct ChainInfo {
    root_name: Option<String>,
    methods: Vec<MethodInfo>,
    span: Span,
}

impl ChainInfo {
    fn from_expr(expr: &Expr) -> Self {
        let mut current = expr;
        let mut methods = Vec::new();
        let mut root_name = None;

        loop {
            match current {
                Expr::MethodCall(method_call) => {
                    methods.push(MethodInfo {
                        name: method_call.method.to_string(),
                        span: method_call.method.span(),
                        args: method_call.args.iter().cloned().collect(),
                    });
                    current = &method_call.receiver;
                }
                Expr::Call(call) => {
                    root_name = called_name(call);
                    break;
                }
                Expr::Path(path) => {
                    root_name = path_name(path);
                    break;
                }
                Expr::Paren(ExprParen { expr, .. }) => {
                    current = expr;
                }
                Expr::Reference(ExprReference { expr, .. }) => {
                    current = expr;
                }
                _ => break,
            }
        }

        methods.reverse();

        Self {
            root_name,
            methods,
            span: expr.span(),
        }
    }

    fn has_layout_container(&self) -> bool {
        self.uses_layout_helper()
            || self
                .methods
                .iter()
                .any(|method| matches!(method.name.as_str(), "flex" | "grid"))
    }

    fn uses_layout_helper(&self) -> bool {
        matches!(self.root_name.as_deref(), Some("h_flex" | "v_flex"))
            || self
                .methods
                .iter()
                .any(|method| matches!(method.name.as_str(), "h_flex" | "v_flex"))
    }

    fn uses_design_system_helper(&self) -> bool {
        matches!(
            self.root_name.as_deref(),
            Some(
                "page_stack"
                    | "page_header"
                    | "page_intro"
                    | "section_stack"
                    | "section_intro"
                    | "section_title"
                    | "field_header"
                    | "info_block"
                    | "empty_hint"
                    | "stat_card"
            )
        )
    }

    fn has_method(&self, name: &str) -> bool {
        self.methods.iter().any(|method| method.name == name)
    }

    fn method(&self, name: &str) -> Option<&MethodInfo> {
        self.methods.iter().find(|method| method.name == name)
    }

    fn child_like_count(&self) -> usize {
        self.methods
            .iter()
            .filter(|method| matches!(method.name.as_str(), "child" | "children"))
            .count()
    }

    fn is_obvious_view_container(&self) -> bool {
        matches!(self.root_name.as_deref(), Some("v_flex" | "panel"))
    }

    fn allows_spacing_method(&self, method_name: &str) -> bool {
        let child_like_count = self.child_like_count();
        match self.root_name.as_deref() {
            Some("panel") => child_like_count >= 1,
            Some("v_flex") => match method_name {
                "gap" => child_like_count >= 2,
                "p" | "px" | "py" => child_like_count >= 1,
                _ => false,
            },
            _ => false,
        }
    }
}

fn called_name(call: &ExprCall) -> Option<String> {
    match call.func.as_ref() {
        Expr::Path(path) => path_name(path),
        Expr::Paren(paren) => match paren.expr.as_ref() {
            Expr::Path(path) => path_name(path),
            _ => None,
        },
        _ => None,
    }
}

fn path_name(path: &ExprPath) -> Option<String> {
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn is_page_spacing_expr(expr: Option<&Expr>) -> bool {
    matches!(
        expr,
        Some(Expr::Call(call))
            if call_name_is(call, &["page_gap", "space_3"]) || is_px_call(call, 12.0)
    )
}

fn is_section_gap_expr(expr: Option<&Expr>) -> bool {
    matches!(
        expr,
        Some(Expr::Call(call))
            if call_name_is(call, &["section_gap", "inline_gap", "space_2"])
                || is_px_call(call, 8.0)
    )
}

fn is_section_padding_expr(expr: Option<&Expr>) -> bool {
    matches!(
        expr,
        Some(Expr::Call(call))
            if call_name_is(call, &["panel_padding", "page_padding", "space_3"])
                || is_px_call(call, 12.0)
    )
}

fn is_raw_spacing_expr(expr: Option<&Expr>) -> bool {
    matches!(
        expr,
        Some(Expr::Call(call))
            if call_name_is(call, &["space_1", "space_2", "space_3"])
                || is_px_call(call, 4.0)
                || is_px_call(call, 8.0)
                || is_px_call(call, 12.0)
    )
}

fn is_px_call(call: &ExprCall, expected: f64) -> bool {
    matches!(
        call.func.as_ref(),
        Expr::Path(path) if matches!(path_name(path).as_deref(), Some("px"))
    ) && call
        .args
        .first()
        .and_then(expr_numeric_value)
        .is_some_and(|value| (value - expected).abs() < f64::EPSILON)
}

fn call_name_is(call: &ExprCall, expected: &[&str]) -> bool {
    called_name(call)
        .as_deref()
        .is_some_and(|name| expected.iter().any(|candidate| *candidate == name))
}

fn expr_numeric_value(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Lit(expr_lit) => match &expr_lit.lit {
            Lit::Int(value) => value.base10_parse().ok(),
            Lit::Float(value) => value.base10_parse().ok(),
            _ => None,
        },
        _ => None,
    }
}

fn method_requires_layout_container(name: &str) -> bool {
    matches!(
        name,
        "flex_col"
            | "flex_col_reverse"
            | "flex_row"
            | "flex_row_reverse"
            | "items_start"
            | "items_end"
            | "items_center"
            | "items_baseline"
            | "items_stretch"
            | "justify_start"
            | "justify_end"
            | "justify_center"
            | "justify_between"
            | "justify_around"
            | "justify_evenly"
    ) || name == "gap"
        || name == "gap_x"
        || name == "gap_y"
        || name.starts_with("gap_")
}

fn lint_source(source: &str, path: &Path) -> Vec<Diagnostic> {
    let parsed = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
    let mut visitor = LayoutLintVisitor {
        file: path,
        diagnostics: Vec::new(),
    };
    visitor.visit_file(&parsed);
    visitor.diagnostics
}

fn lint_repo_sources() -> Vec<Diagnostic> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);

    let mut diagnostics = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", file.display()));
        diagnostics.extend(lint_source(&source, &file));
    }
    diagnostics
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
        .map(|entry| entry.expect("failed to read directory entry").path())
        .collect();
    entries.sort();

    for entry in entries {
        if entry.is_dir() {
            collect_rs_files(&entry, out);
        } else if entry.extension().is_some_and(|ext| ext == "rs") {
            out.push(entry);
        }
    }
}

fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut output = String::new();

    for diagnostic in diagnostics {
        let display_path = diagnostic
            .file
            .strip_prefix(&manifest_dir)
            .unwrap_or(&diagnostic.file);
        let _ = writeln!(
            output,
            "{}:{}:{} [{}] {}",
            display_path.display(),
            diagnostic.line,
            diagnostic.column,
            diagnostic.rule,
            diagnostic.message
        );
    }

    output
}

#[test]
fn repo_gpui_layout_chains_follow_lint_rules() {
    let diagnostics = lint_repo_sources();
    assert!(
        diagnostics.is_empty(),
        "GPUI layout lint failed:\n{}",
        format_diagnostics(&diagnostics)
    );
}

#[test]
fn flags_missing_flex_before_flex_col() {
    let diagnostics = lint_source(
        r#"
        fn render() {
            div()
                .flex_col()
                .gap(px(8.0))
                .child("broken");
        }
        "#,
        Path::new("snippet.rs"),
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "missing-layout-container"),
        "expected missing-layout-container diagnostic, got: {:?}",
        diagnostics
    );
}

#[test]
fn flags_prefer_v_flex_for_plain_column_stack() {
    let diagnostics = lint_source(
        r#"
        fn render() {
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child("stack");
        }
        "#,
        Path::new("snippet.rs"),
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "prefer-v-flex"),
        "expected prefer-v-flex diagnostic, got: {:?}",
        diagnostics
    );
}

#[test]
fn accepts_v_flex_helpers() {
    let diagnostics = lint_source(
        r#"
        fn render() {
            v_flex()
                .gap(px(8.0))
                .child("ok");
        }
        "#,
        Path::new("snippet.rs"),
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        diagnostics
    );
}

#[test]
fn flags_prefer_page_stack_for_manual_page_wrapper() {
    let diagnostics = lint_source(
        r#"
        fn render() {
            v_flex()
                .gap(theme::space_3())
                .child("header")
                .child("body");
        }
        "#,
        Path::new("src/views/snippet.rs"),
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "prefer-page-stack"),
        "expected prefer-page-stack diagnostic, got: {:?}",
        diagnostics
    );
}

#[test]
fn accepts_page_stack_helper() {
    let diagnostics = lint_source(
        r#"
        fn render() {
            page_stack()
                .child("header")
                .child("body");
        }
        "#,
        Path::new("src/views/snippet.rs"),
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        diagnostics
    );
}

#[test]
fn flags_prefer_section_stack_for_manual_section_wrapper() {
    let diagnostics = lint_source(
        r#"
        fn render() {
            panel(1)
                .p(theme::space_3())
                .v_flex()
                .gap(theme::space_2())
                .child("body");
        }
        "#,
        Path::new("src/views/snippet.rs"),
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "prefer-section-stack"),
        "expected prefer-section-stack diagnostic, got: {:?}",
        diagnostics
    );
}

#[test]
fn accepts_section_stack_helper() {
    let diagnostics = lint_source(
        r#"
        fn render() {
            section_stack(1)
                .child("body");
        }
        "#,
        Path::new("src/views/snippet.rs"),
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        diagnostics
    );
}

#[test]
fn flags_prefer_semantic_spacing_tokens_for_vertical_view_container() {
    let diagnostics = lint_source(
        r#"
        fn render() {
            v_flex()
                .gap(theme::space_2())
                .child("header")
                .child("body");
        }
        "#,
        Path::new("src/views/snippet.rs"),
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "prefer-semantic-spacing-token"),
        "expected prefer-semantic-spacing-token diagnostic, got: {:?}",
        diagnostics
    );
}

#[test]
fn accepts_horizontal_rows_with_raw_spacing() {
    let diagnostics = lint_source(
        r#"
        fn render() {
            h_flex()
                .gap(px(8.0))
                .child("left")
                .child("right");
        }
        "#,
        Path::new("src/views/snippet.rs"),
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        diagnostics
    );
}
