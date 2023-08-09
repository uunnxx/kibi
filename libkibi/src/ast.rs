

//
// common
//

#[derive(Clone, Copy, Debug)]
pub struct Span<'a> {
    pub value: &'a str,
}



//
// tokens
//

#[derive(Clone, Copy, Debug)]
pub struct Token<'a> {
    p: core::marker::PhantomData<&'a ()>,
}



//
// ast common
//

#[derive(Clone, Copy, Debug)]
pub struct Ident<'a> {
    pub span: Span<'a>
}

#[derive(Clone, Copy, Debug)]
pub struct Path<'a> {
    pub span: Span<'a>
}

#[derive(Clone, Copy, Debug)]
pub enum GenericIdent<'a> {
    Ident(Ident<'a>),
    DotIdent(Ident<'a>),
    Path(Path<'a>),
}



//
// items
//

#[derive(Debug)]
pub struct Item<'a> {
    pub kind: ItemKind<'a>,
}

#[derive(Debug)]
pub enum ItemKind<'a> {
    Use(Path<'a>),
}



//
// stmts
//

pub type StmtRef<'a> = &'a mut Stmt<'a>;

pub type StmtList<'a> = &'a mut [StmtRef<'a>];

#[derive(Debug)]
pub struct Stmt<'a> {
    pub kind: StmtKind<'a>,
}

#[derive(Debug)]
pub enum StmtKind<'a> {
    Let(stmt::Let<'a>),
    Expr(Expr<'a>),
}


pub mod stmt {
    use super::*;


    #[derive(Debug)]
    pub struct Let<'a> {
        pub is_var: bool,
        pub ident:  Ident<'a>,
        pub ty:     Option<TypeRef<'a>>,
        pub value:  Option<ExprRef<'a>>,
    }
}



//
// types
//

pub type TypeRef<'a> = &'a mut Type<'a>;

pub type TypeList<'a> = &'a mut [TypeRef<'a>];

#[derive(Debug)]
pub struct Type<'a> {
    pub kind: TypeKind<'a>,
}

#[derive(Debug)]
pub enum TypeKind<'a> {
    Ident(Ident<'a>),
    Path(Path<'a>),
    Array(TypeRef<'a>),
    Map(ty::Map<'a>),
}


pub mod ty {
    use super::*;


    #[derive(Debug)]
    pub struct Map<'a> {
        pub key:   TypeRef<'a>,
        pub value: TypeRef<'a>,
    }
}



//
// exprs
//

pub type ExprRef<'a> = &'a mut Expr<'a>;

pub type ExprList<'a> = &'a mut [ExprRef<'a>];

#[derive(Debug)]
pub struct Expr<'a> {
    pub kind: ExprKind<'a>,
}

#[derive(Debug)]
pub enum ExprKind<'a> {
    Ident(Ident<'a>),
    DotIdent(Ident<'a>),
    Path(Path<'a>),

    Parens(ExprRef<'a>),
    Block(expr::Block<'a>),

    Op1(expr::Op1<'a>),
    Op2(expr::Op2<'a>),

    Field(expr::Field<'a>),
    Index(expr::Index<'a>),
    Call(expr::Call<'a>),

    Assign(expr::Assign<'a>),

    Array(expr::Array<'a>),
    Map(expr::Map<'a>),

    Match(expr::Match<'a>),
    If(expr::If<'a>),
    Loop(expr::Loop<'a>),

    TypeHint(expr::TypeHint<'a>),
}


pub mod expr {
    use super::*;


    #[derive(Debug)]
    pub struct Block<'a> {
        pub is_do: bool,
        pub stmts: StmtList<'a>,
    }


    #[derive(Debug)]
    pub enum Op1Kind {
    }

    #[derive(Debug)]
    pub struct Op1<'a> {
        pub op:   Op1Kind,
        pub expr: ExprRef<'a>,
    }


    #[derive(Debug)]
    pub enum Op2Kind {
    }

    #[derive(Debug)]
    pub struct Op2<'a> {
        pub op:  Op2Kind,
        pub lhs: ExprRef<'a>,
        pub rhs: ExprRef<'a>,
    }


    #[derive(Debug)]
    pub struct Field<'a> {
        pub base: ExprRef<'a>,
        pub name: Ident<'a>,
    }

    #[derive(Debug)]
    pub struct Index<'a> {
        pub base:  ExprRef<'a>,
        pub index: ExprRef<'a>,
    }

    #[derive(Debug)]
    pub struct Call<'a> {
        pub self_post_eval: bool,
        pub func: ExprRef<'a>,
        pub args: CallArgList<'a>,
    }

    pub type CallArgList<'a> = &'a mut [CallArg<'a>];

    #[derive(Debug)]
    pub enum CallArg<'a> {
        Positional(ExprRef<'a>),
        Named(NamedArg<'a>),
    }

    #[derive(Debug)]
    pub struct NamedArg<'a> {
        pub name:  Ident<'a>,
        pub value: ExprRef<'a>,
    }


    #[derive(Debug)]
    pub struct Assign<'a> {
        pub lhs: ExprRef<'a>,
        pub rhs: ExprRef<'a>,
    }


    #[derive(Debug)]
    pub struct Array<'a> {
        pub values: ExprList<'a>,
    }

    #[derive(Debug)]
    pub struct Map<'a> {
        pub entries: MapEntryList<'a>,
    }

    pub type MapEntryList<'a> = &'a mut [MapEntry<'a>];

    #[derive(Debug)]
    pub struct MapEntry<'a> {
        pub key:   Ident<'a>,
        pub value: ExprRef<'a>,
    }


    #[derive(Debug)]
    pub struct Match<'a> {
        pub expr: ExprRef<'a>,
        pub cases: MatchCaseList<'a>,
    }

    pub type MatchCaseList<'a> = &'a mut [MatchCase<'a>];

    #[derive(Debug)]
    pub struct MatchCase<'a> {
        pub ctor:    GenericIdent<'a>,
        pub binding: Option<Ident<'a>>,
        pub expr:    ExprRef<'a>,
    }


    #[derive(Debug)]
    pub struct If<'a> {
        pub cond:  ExprRef<'a>,
        pub then:  Block<'a>,
        pub elifs: IfCaseList<'a>,
        pub els:   Option<ExprRef<'a>>,
    }

    pub type IfCaseList<'a> = &'a mut [IfCase<'a>];

    #[derive(Debug)]
    pub struct IfCase<'a> {
        pub cond: ExprRef<'a>,
        pub then: Block<'a>,
    }


    #[derive(Debug)]
    pub struct Loop<'a> {
        pub cond: Option<ExprRef<'a>>,
        pub body: Block<'a>,
    }


    #[derive(Debug)]
    pub struct TypeHint<'a> {
        pub expr: ExprRef<'a>,
        pub ty:   TypeRef<'a>,
    }
}

