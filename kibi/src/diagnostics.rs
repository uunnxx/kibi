use sti::vec::Vec;

use crate::ast::{ParseRange, TokenId, TokenRange, ItemId, StmtId, LevelId, ExprId, SourceRange};
use crate::tt;
use crate::env::SymbolId;
use crate::parser::Parse;
use crate::pp::DocRef;


pub struct Diagnostics<'a> {
    pub diagnostics: Vec<Diagnostic<'a>>,
}


#[derive(Debug)]
pub struct Diagnostic<'a> {
    pub source: DiagnosticSource,
    pub kind: DiagnosticKind<'a>,
}

#[derive(Clone, Copy, Debug)]
pub enum DiagnosticSource {
    ParseRange(ParseRange),
    Token(TokenId),
    TokenRange(TokenRange),
    Item(ItemId),
    Stmt(StmtId),
    Level(LevelId),
    Expr(ExprId),
    Unknown, // @temp: tyck errors should fall back to itemid.
}

#[derive(Debug)]
pub enum DiagnosticKind<'a> {
    ParseError(ParseError<'a>),
    ElabError(ElabError<'a>),
    TyCkError(TyCkError<'a>),
}


#[derive(Clone, Copy, Debug)]
pub enum ParseError<'a> {
    Expected(&'a str),
    Unexpected(&'a str),
}


#[derive(Clone, Copy, Debug)]
pub enum ElabError<'a> {
    UnresolvedName  (&'a str),
    UnresolvedLevel (&'a str),
    LevelCountMismatch { expected: u32, found: u32 },
    TypeMismatch { expected: DocRef<'a>, found: DocRef<'a> },
    TypeExpected { found: DocRef<'a> },
    TooManyArgs,
    UnassignedIvars,
    TypeFormerHasIvars,
    CtorTypeHasIvars,
    CtorNeedsTypeCauseIndices,
    CtorArgLevelTooLarge,
    CtorInvalidRecursion,
    CtorRecArgUsed,
    CtorNotRetSelf,
    TraitResolutionFailed { trayt: SymbolId },
    ImplTypeIsNotTrait,

    // @temp
    TempTBD,
    TempArgFailed,
    TempCtorArgLevelCouldBeTooLarge,
    TempUnimplemented,
    TempStr(&'a str),
}

#[derive(Debug)]
pub struct TyCkError<'a> {
    pub lctx: tt::LocalCtx<'a>,
    pub err:  tt::tyck::Error<'a>,
}


impl<'a> Diagnostics<'a> {
    #[inline(always)]
    pub fn new() -> Self {
        Self { diagnostics: Vec::new() }
    }

    #[inline(always)]
    pub fn push(&mut self, diagnostic: Diagnostic<'a>) {
        self.diagnostics.push(diagnostic);
    }
}


impl DiagnosticSource {
    pub fn resolve_source_range(self, parse: &Parse) -> SourceRange {
        use DiagnosticSource as DS;
        match self {
            DS::ParseRange(it) => parse.resolve_parse_range(it),
            DS::Token(it)      => parse.resolve_token_range(TokenRange::from_key(it)),
            DS::TokenRange(it) => parse.resolve_token_range(it),
            DS::Item(it)  => parse.resolve_token_range(parse.items[it].source),
            DS::Stmt(it)  => parse.resolve_token_range(parse.stmts[it].source),
            DS::Level(it) => parse.resolve_token_range(parse.levels[it].source),
            DS::Expr(it)  => parse.resolve_token_range(parse.exprs[it].source),
            DS::Unknown => SourceRange { begin: 0, end: 0 },
        }
    }
}


impl Into<DiagnosticSource> for ParseRange { #[inline(always)] fn into(self) -> DiagnosticSource { DiagnosticSource::ParseRange(self) } }
impl Into<DiagnosticSource> for TokenId    { #[inline(always)] fn into(self) -> DiagnosticSource { DiagnosticSource::Token(self)      } }
impl Into<DiagnosticSource> for TokenRange { #[inline(always)] fn into(self) -> DiagnosticSource { DiagnosticSource::TokenRange(self) } }
impl Into<DiagnosticSource> for ItemId     { #[inline(always)] fn into(self) -> DiagnosticSource { DiagnosticSource::Item(self)       } }
impl Into<DiagnosticSource> for StmtId     { #[inline(always)] fn into(self) -> DiagnosticSource { DiagnosticSource::Stmt(self)       } }
impl Into<DiagnosticSource> for LevelId    { #[inline(always)] fn into(self) -> DiagnosticSource { DiagnosticSource::Level(self)      } }
impl Into<DiagnosticSource> for ExprId     { #[inline(always)] fn into(self) -> DiagnosticSource { DiagnosticSource::Expr(self)       } }

