//! 本物の構文解析(oxc)で、TypeScript のファイルを「世界の木」に直す。
//! 木の節は、関数・文・分岐・ループ・ブロック・型・import。行数と元の文字は本物から取る。
//! 節ごとに、宣言している名前と、使っている名前(と、その使い方)も集める。依存の線の元になる。

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
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

/// 依存の種類(依存の線の姿を分ける)。同じ相手を何通りにも使っていたら、後ろのもの(強いもの)を残す
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum How {
    /// 型として使うだけ(動くときには消える)
    Type,
    /// 取り込み(import、package.json の依存)
    Import,
    /// 値を読む
    Read,
    /// 関数を呼ぶ
    Call,
    /// 値を書き換える
    Write,
}

/// 入れ物の中身を変える呼び出し(`list.push(x)` は、list を書き換えている)
const MUTATING: [&str; 14] = ["push", "pop", "shift", "unshift", "splice", "sort", "reverse", "fill", "set", "add", "delete", "clear", "copyWithin", "length"];
/// プログラムの外に触る物: いつでも使える名前と、Node.js に備え付けのモジュール(ネットワーク、ファイル、プロセス)
const OUTSIDE_NAMES: [&str; 4] = ["fetch", "process", "WebSocket", "XMLHttpRequest"];
const OUTSIDE_MODULES: [&str; 13] = ["fs", "fs/promises", "http", "https", "http2", "net", "dgram", "tls", "dns", "child_process", "os", "readline", "worker_threads"];

#[derive(Debug)]
pub struct Node {
    pub kind: Kind,
    /// 画面に出す名前(関数名、または元のコードの 1 行目)
    pub label: String,
    /// 元のコードで何行ぶんか
    pub lines: usize,
    pub kids: Vec<Node>,
    /// この節が宣言している名前(関数名、変数名、型名、import した名前)
    pub declares: Vec<String>,
    /// この節の中で使っている名前と、その使い方
    pub uses: Vec<(String, How)>,
    /// プログラムの外に触っているか
    pub outside: bool,
}

/// 節の中を歩いて、使っている名前を集める。名前が指す先までは調べない(同じ名前の別の物を、取り違えることがある)
#[derive(Default)]
struct Uses {
    found: Vec<(String, How)>,
    outside: bool,
}

impl Uses {
    fn add(&mut self, name: &str, how: How) {
        self.outside |= OUTSIDE_NAMES.contains(&name);
        match self.found.iter_mut().find(|u| u.0 == name) {
            Some(u) => u.1 = u.1.max(how),
            None => self.found.push((name.to_string(), how)),
        }
    }

    fn into(self, mut node: Node) -> Node {
        node.outside |= self.outside;
        node.uses = self.found;
        node
    }
}

/// `a.b.c` の、いちばん元の名前(a)
fn root<'e>(e: &'e Expression) -> Option<&'e str> {
    match e {
        Expression::Identifier(id) => Some(id.name.as_str()),
        Expression::StaticMemberExpression(m) => root(&m.object),
        Expression::ComputedMemberExpression(m) => root(&m.object),
        _ => None,
    }
}

impl<'a> Visit<'a> for Uses {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        self.add(it.name.as_str(), How::Read);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        match &it.callee {
            Expression::Identifier(id) => self.add(id.name.as_str(), How::Call),
            Expression::StaticMemberExpression(m) => {
                if let Some(name) = root(&m.object) {
                    self.add(name, if MUTATING.contains(&m.property.name.as_str()) { How::Write } else { How::Call });
                }
            }
            _ => {}
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        let name = match &it.left {
            AssignmentTarget::AssignmentTargetIdentifier(id) => Some(id.name.as_str()),
            AssignmentTarget::StaticMemberExpression(m) => root(&m.object),
            AssignmentTarget::ComputedMemberExpression(m) => root(&m.object),
            _ => None,
        };
        if let Some(name) = name {
            self.add(name, How::Write);
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_update_expression(&mut self, it: &UpdateExpression<'a>) {
        let name = match &it.argument {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => Some(id.name.as_str()),
            SimpleAssignmentTarget::StaticMemberExpression(m) => root(&m.object),
            SimpleAssignmentTarget::ComputedMemberExpression(m) => root(&m.object),
            _ => None,
        };
        if let Some(name) = name {
            self.add(name, How::Write);
        }
        walk::walk_update_expression(self, it);
    }

    fn visit_ts_type_name(&mut self, it: &TSTypeName<'a>) {
        // 型の名前は、型としてだけ数える(中へは進まない)
        if let TSTypeName::IdentifierReference(id) = it {
            self.add(id.name.as_str(), How::Type);
        }
    }
}

/// 文が宣言している名前
fn declares(s: &Statement) -> Vec<String> {
    let of = |d: &Declaration| -> Vec<String> {
        match d {
            Declaration::FunctionDeclaration(f) => f.id.iter().map(|i| i.name.to_string()).collect(),
            Declaration::VariableDeclaration(v) => v.declarations.iter().flat_map(|d| d.id.get_binding_identifiers()).map(|i| i.name.to_string()).collect(),
            Declaration::TSInterfaceDeclaration(t) => vec![t.id.name.to_string()],
            Declaration::TSTypeAliasDeclaration(t) => vec![t.id.name.to_string()],
            Declaration::ClassDeclaration(c) => c.id.iter().map(|i| i.name.to_string()).collect(),
            _ => vec![],
        }
    };
    match s {
        Statement::ImportDeclaration(i) => i.specifiers.iter().flatten().map(|sp| sp.local().name.to_string()).collect(),
        Statement::ExportDeclaration(e) => of(&e.declaration),
        _ => s.as_declaration().map(of).unwrap_or_default(),
    }
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
        self.node(kind, self.first_line(span), span, vec![])
    }
    fn node(&self, kind: Kind, label: String, span: Span, kids: Vec<Node>) -> Node {
        Node { kind, label, lines: self.lines(span), kids, declares: vec![], uses: vec![], outside: false }
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
        let mut uses = Uses::default();
        if let Some(b) = body {
            uses.visit_function_body(b);
        }
        uses.into(self.node(Kind::Fn, name, span, kids))
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

    /// 文を 1 つ読む。宣言している名前と、使っている名前も付ける
    fn stmt(&self, s: &Statement) -> Node {
        let mut uses = Uses::default();
        uses.visit_statement(s);
        let mut node = uses.into(self.shape(s));
        node.declares = declares(s);
        node
    }

    fn shape(&self, s: &Statement) -> Node {
        match s {
            Statement::ImportDeclaration(i) => {
                let mut node = self.leaf(Kind::Import, i.span);
                let from = i.source.value.as_str().unwrap_or("");
                node.outside = OUTSIDE_MODULES.contains(&from.strip_prefix("node:").unwrap_or(from));
                node
            }
            Statement::ExportDeclaration(e) => self.declaration(&e.declaration),
            Statement::FunctionDeclaration(_)
            | Statement::VariableDeclaration(_)
            | Statement::TSInterfaceDeclaration(_)
            | Statement::TSTypeAliasDeclaration(_) => self.declaration(s.as_declaration().unwrap()),
            Statement::IfStatement(i) => {
                let mut uses = Uses::default();
                uses.visit_expression(&i.test);
                let mut cond = uses.into(self.leaf(Kind::Cond, i.test.span()));
                cond.lines = 1;
                let mut kids = vec![cond];
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
    Ok(Node { kind: Kind::File, label, lines: src.lines().count(), kids, declares: vec![], uses: vec![], outside: false })
}
