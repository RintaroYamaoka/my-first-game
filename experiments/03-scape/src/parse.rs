//! 本物の構文解析(oxc)で、TypeScript のファイルを「世界の木」に直す。
//! 木の節は、関数・文・分岐・ループ・ブロック・型・import。行数と元の文字は本物から取る。

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    File,
    Import,
    Fn,
    Stmt,
    Return,
    Type,
    Branch, // 子: 条件、then のブロック、(あれば)else のブロック
    Cond,
    Block,
    Loop, // 子: 繰り返す中身
    Try,  // 子: try / catch / finally のブロック
}

#[derive(Debug)]
pub struct Node {
    pub kind: Kind,
    /// 画面に出す名前(関数名、または元のコードの 1 行目)
    pub label: String,
    /// 元のコードで何行ぶんか
    pub lines: usize,
    pub kids: Vec<Node>,
}

struct Ctx<'s> {
    src: &'s str,
}

impl<'s> Ctx<'s> {
    fn text(&self, span: Span) -> &'s str {
        &self.src[span.start as usize..span.end as usize]
    }
    fn first_line(&self, span: Span) -> String {
        let line = self.text(span).lines().next().unwrap_or("").trim();
        let mut s: String = line.chars().take(46).collect();
        if line.chars().count() > 46 {
            s.push('…');
        }
        s
    }
    fn lines(&self, span: Span) -> usize {
        self.text(span).lines().count().max(1)
    }
    fn leaf(&self, kind: Kind, span: Span) -> Node {
        Node { kind, label: self.first_line(span), lines: self.lines(span), kids: vec![] }
    }
    fn node(&self, kind: Kind, label: String, span: Span, kids: Vec<Node>) -> Node {
        Node { kind, label, lines: self.lines(span), kids }
    }

    fn stmts(&self, list: &[Statement]) -> Vec<Node> {
        list.iter().map(|s| self.stmt(s)).collect()
    }

    /// 文を 1 つ、ブロックとして読む(`{ ... }` でも、1 文だけでも)
    fn block(&self, label: &str, s: &Statement) -> Node {
        let kids = match s {
            Statement::BlockStatement(b) => self.stmts(&b.body),
            other => vec![self.stmt(other)],
        };
        self.node(Kind::Block, label.into(), s.span(), kids)
    }

    fn function(&self, name: String, span: Span, body: Option<&FunctionBody>) -> Node {
        let kids = body.map(|b| self.stmts(&b.statements)).unwrap_or_default();
        self.node(Kind::Fn, name, span, kids)
    }

    fn arrow(&self, name: String, span: Span, a: &ArrowFunctionExpression) -> Node {
        match &a.body {
            ArrowFunctionBody::FunctionBody(b) => self.function(name, span, Some(&**b)),
            _ => self.function(name, span, None), // 式を 1 つ返すだけの関数
        }
    }

    /// 式の中に書かれた関数(コールバック)を拾う。例: test("...", () => { ... })
    fn callbacks(&self, e: &Expression) -> Vec<Node> {
        let Expression::CallExpression(call) = e else { return vec![] };
        let mut out = vec![];
        for arg in &call.arguments {
            match arg {
                Argument::ArrowFunctionExpression(a) => out.push(self.arrow("() =>".into(), a.span, a)),
                Argument::FunctionExpression(f) => out.push(self.function("function".into(), f.span, f.body.as_deref())),
                _ => {}
            }
        }
        out
    }

    fn declaration(&self, d: &Declaration) -> Node {
        match d {
            Declaration::FunctionDeclaration(f) => {
                let name = f.id.as_ref().map(|i| format!("{}()", i.name)).unwrap_or_else(|| "function".into());
                self.function(name, f.span, f.body.as_deref())
            }
            Declaration::VariableDeclaration(v) => {
                // const f = () => { ... } は関数として扱う
                if let [one] = v.declarations.as_slice() {
                    match &one.init {
                        Some(Expression::ArrowFunctionExpression(a)) => {
                            return self.arrow(format!("{}()", self.text(one.id.span())), v.span, a);
                        }
                        Some(Expression::FunctionExpression(f)) => {
                            return self.function(format!("{}()", self.text(one.id.span())), v.span, f.body.as_deref());
                        }
                        Some(init) => {
                            let kids = self.callbacks(init);
                            if !kids.is_empty() {
                                return self.node(Kind::Stmt, self.first_line(v.span), v.span, kids);
                            }
                        }
                        None => {}
                    }
                }
                self.leaf(Kind::Stmt, v.span)
            }
            Declaration::TSInterfaceDeclaration(t) => self.leaf(Kind::Type, t.span),
            Declaration::TSTypeAliasDeclaration(t) => self.leaf(Kind::Type, t.span),
            other => self.leaf(Kind::Stmt, other.span()),
        }
    }

    fn stmt(&self, s: &Statement) -> Node {
        match s {
            Statement::ImportDeclaration(i) => self.leaf(Kind::Import, i.span),
            Statement::ExportDeclaration(e) => self.declaration(&e.declaration),
            Statement::FunctionDeclaration(_)
            | Statement::VariableDeclaration(_)
            | Statement::TSInterfaceDeclaration(_)
            | Statement::TSTypeAliasDeclaration(_) => self.declaration(s.as_declaration().unwrap()),
            Statement::IfStatement(i) => {
                let mut kids = vec![Node { kind: Kind::Cond, label: self.first_line(i.test.span()), lines: 1, kids: vec![] }];
                kids.push(self.block("then", &i.consequent));
                if let Some(alt) = &i.alternate {
                    kids.push(self.block("else", alt));
                }
                self.node(Kind::Branch, format!("if ({})", self.first_line(i.test.span())), i.span, kids)
            }
            Statement::ForStatement(f) => self.looped(f.span, &f.body),
            Statement::ForInStatement(f) => self.looped(f.span, &f.body),
            Statement::ForOfStatement(f) => self.looped(f.span, &f.body),
            Statement::WhileStatement(w) => self.looped(w.span, &w.body),
            Statement::DoWhileStatement(w) => self.looped(w.span, &w.body),
            Statement::BlockStatement(b) => self.node(Kind::Block, "{ }".into(), b.span, self.stmts(&b.body)),
            Statement::TryStatement(t) => {
                let mut kids = vec![self.node(Kind::Block, "try".into(), t.block.span, self.stmts(&t.block.body))];
                if let Some(h) = &t.handler {
                    kids.push(self.node(Kind::Block, "catch".into(), h.span, self.stmts(&h.body.body)));
                }
                if let Some(f) = &t.finalizer {
                    kids.push(self.node(Kind::Block, "finally".into(), f.span, self.stmts(&f.body)));
                }
                self.node(Kind::Try, "try".into(), t.span, kids)
            }
            Statement::ReturnStatement(r) => self.leaf(Kind::Return, r.span),
            Statement::ExpressionStatement(e) => {
                let kids = self.callbacks(&e.expression);
                self.node(Kind::Stmt, self.first_line(e.span), e.span, kids)
            }
            other => self.leaf(Kind::Stmt, other.span()),
        }
    }

    fn looped(&self, span: Span, body: &Statement) -> Node {
        let kids = match body {
            Statement::BlockStatement(b) => self.stmts(&b.body),
            other => vec![self.stmt(other)],
        };
        self.node(Kind::Loop, self.first_line(span).trim_end_matches('{').trim().to_string(), span, kids)
    }
}

pub fn parse_file(path: &Path, label: String) -> Result<Node, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let source_type = SourceType::from_path(path).map_err(|_| format!("{}: 種類が分からない", path.display()))?;
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, &src, source_type).parse();
    if ret.fatal_error || ret.diagnostics.has_errors() {
        return Err(format!("{}: 構文エラーで読めない", path.display()));
    }
    let ctx = Ctx { src: &src };
    let kids = ctx.stmts(&ret.program.body);
    Ok(Node { kind: Kind::File, label, lines: src.lines().count(), kids })
}

/// 読み取った木を、文字で確かめる
pub fn report(n: &Node, depth: usize) {
    println!("{}{:<7} {:>3} 行  {}", "  ".repeat(depth), format!("{:?}", n.kind), n.lines, n.label);
    for k in &n.kids {
        report(k, depth + 1);
    }
}
