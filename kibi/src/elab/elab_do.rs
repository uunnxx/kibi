use sti::traits::CopyIt;

use crate::ast::*;
use crate::tt::*;

use super::*;

impl<'me, 'c, 'out> Elaborator<'me, 'c, 'out> {
    pub fn elab_do(&mut self, expr_id: ExprId, flags: ExprFlags, block: expr::Block, expected_ty: Option<Term<'out>>)
        -> Option<(Term<'out>, Term<'out>)>
    {
        let may_need_joins = flags.has_loop || flags.has_jump || (flags.has_if && flags.has_assign);
        if !may_need_joins {
            if block.stmts.len() == 0 {
                return Some((Term::UNIT_MK, Term::UNIT));
            }
            else if block.stmts.len() == 1 {
                let stmt = block.stmts[0];
                if let StmtKind::Expr(it) = self.parse.stmts[stmt].kind {
                    // @todo: expected type.
                    return self.elab_expr(it);
                }
            }

            // @todo: elab_let_chain.
            // chain also for regular let, so we can do the multi-abstract thing.
            self.error(expr_id, ElabError::TempStr("unimp do without joins"));
            None
        }
        else {
            println!("---\n");

            let old_scope = self.lctx.current;
            let old_locals = self.locals.clone();

            let result_ty = expected_ty.unwrap_or_else(|| self.new_term_var().0);

            let mut this = ElabDo::new(self, result_ty);
            let entry = this.new_jp_with_locals(false);
            this.begin_jp(entry);
            let (ret, ret_ty) = this.elab_do_block(expr_id, flags, block, Some(result_ty))?;
            // @cleanup: dedup.
            if !this.ensure_def_eq(ret_ty, result_ty) {
                let expected = this.instantiate_term_vars(result_ty);
                let ty       = this.instantiate_term_vars(ret_ty);
                let expected = this.reduce_ex(expected, false);
                let ty       = this.reduce_ex(ty, false);
                this.elab.error(expr_id, {
                    let mut pp = TermPP::new(this.elab.env, &this.elab.strings, &this.elab.lctx, this.elab.alloc);
                    let expected = pp.pp_term(expected);
                    let found    = pp.pp_term(ty);
                    ElabError::TypeMismatch { expected, found }
                });
                return None;
            }
            this.end_jp(ret);

            for (_, jp) in this.jps.iter() {
                let (term, ty) = if jp.reachable {
                    let (term, ty) = jp.term.unwrap();

                    let term = this.elab.instantiate_term_vars(term);
                    let ty   = this.elab.instantiate_term_vars(ty);
                    if term.has_ivars() || ty.has_ivars() {
                        self.error(expr_id, ElabError::TempStr("jp has ivars"));
                        return None;
                    }
                    (term, ty)
                }
                else {
                    let name = &this.elab.strings[this.elab.env.symbol(jp.symbol_id).name];
                    println!("{}: unreachable\n", name);
                    (this.elab.mkt_ax_unreach(result_ty), result_ty)
                };

                this.elab.env.resolve_pending(jp.symbol_id, SymbolKind::Def(symbol::Def {
                    val: Some(term),
                    ty,
                    num_levels: 0, // @temp: jp levels.
                }));
            }

            let entry = &this.jps[entry];
            let entry_id = entry.symbol_id;

            self.lctx.current = old_scope;
            // @cleanup: sti vec assign/_from_slice.
            self.locals.clear();
            self.locals.extend_from_slice(&old_locals);

            let local_terms = Vec::from_iter(self.locals.copy_map_it(|(_, local)| self.alloc.mkt_local(local)));
            let result = self.alloc.mkt_apps(self.alloc.mkt_global(entry_id, &[]), &local_terms);

            Some((result, result_ty))
        }
    }
}


sti::define_key!(u32, JoinPoint, opt: OptJoinPoint);

struct ElabDo<'me, 'e, 'c, 'out> {
    elab: &'me mut Elaborator<'e, 'c, 'out>,

    jps: KVec<JoinPoint, JoinPointData<'out>>,
    result_ty: Term<'out>,

    jump_targets: Vec<JumpTarget<'out>>,

    current_jp: OptJoinPoint,
    stmts: Vec<Stmt<'out>>,
}

#[derive(Clone, Copy)]
struct JumpTarget<'a> {
    jp: JoinPoint,
    result_ty: Option<Term<'a>>,
}

struct JoinPointData<'a> {
    symbol_id: SymbolId,
    symbol: Term<'a>,
    params: Vec<ScopeId>,
    needs_value: bool,
    term: Option<(Term<'a>, Term<'a>)>,
    reachable: bool,
}

#[derive(Clone, Copy)]
enum Stmt<'out> {
    Local(ScopeId),
    Term((Term<'out>, Term<'out>)),
}

impl<'me, 'e, 'c, 'out> ElabDo<'me, 'e, 'c, 'out> {
    fn new(elab: &'me mut Elaborator<'e, 'c, 'out>, result_ty: Term<'out>) -> Self {
        ElabDo {
            elab,
            jps: KVec::new(),
            jump_targets: Vec::new(),
            result_ty,
            current_jp: None.into(),
            stmts: Vec::new(),
        }
    }

    fn new_jp_with_locals(&mut self, needs_value: bool) -> JoinPoint {
        // @temp: temp alloc, symbols under current symbol.
        let n = self.env.symbol(SymbolId::ROOT).children.len();
        let name = self.strings.insert(&sti::format!("__jp_{}", n));
        let params = Vec::from_iter(self.locals.copy_map_it(|(_, id)| id));
        let symbol_id = self.env.new_symbol(SymbolId::ROOT, name, SymbolKind::Pending).unwrap();
        let symbol = self.alloc.mkt_global(symbol_id, &[]); // @temp: levels.
        self.jps.push(JoinPointData { symbol_id, symbol, params, needs_value, term: None, reachable: true })
    }

    fn new_jp_with_params_of(&mut self, jp: JoinPoint, needs_value: bool) -> JoinPoint {
        // @temp: temp alloc, symbols under current symbol.
        let n = self.env.symbol(SymbolId::ROOT).children.len();
        let name = self.strings.insert(&sti::format!("__jp_{}", n));
        let params = self.jps[jp].params.clone();
        let symbol_id = self.env.new_symbol(SymbolId::ROOT, name, SymbolKind::Pending).unwrap();
        let symbol = self.alloc.mkt_global(symbol_id, &[]); // @temp: levels.
        self.jps.push(JoinPointData { symbol_id, symbol, params, needs_value, term: None, reachable: true })
    }

    fn begin_jp(&mut self, jp: JoinPoint) {
        assert!(self.current_jp.is_none());
        assert_eq!(self.stmts.len(), 0);

        // parameterize locals
        self.elab.lctx.current = None.into();
        self.elab.locals.clear();
        for id in &mut self.jps[jp].params {
            let entry = self.elab.lctx.lookup(*id).clone();
            *id = self.elab.lctx.push(entry.binder_kind, entry.name, entry.ty, None);
            self.elab.locals.push((entry.name, *id));
        }

        self.current_jp = jp.some();
    }

    fn mk_jump(&self, jp: JoinPoint, value: Option<Term<'out>>) -> Term<'out> {
        let target = &self.jps[jp];
        assert!(target.needs_value == value.is_some());

        let locals = &self.locals[..target.params.len()];
        let locals = Vec::from_iter(locals.copy_map_it(|(_, local)| self.alloc.mkt_local(local)));
        let result = self.alloc.mkt_apps(target.symbol, &locals);
        if let Some(value) = value {
            self.alloc.mkt_apply(result, value)
        }
        else { result }
    }

    fn end_jp(&mut self, ret: Term<'out>) {
        let entry = &mut self.jps[self.current_jp.unwrap()];
        assert!(entry.term.is_none());

        let mut result    = ret;
        let mut result_ty = self.result_ty;
        for stmt in self.stmts.copy_it().rev() {
            match stmt {
                Stmt::Local(id) => {
                    result    = self.elab.mk_let(result,    id, false);
                    result_ty = self.elab.mk_let(result_ty, id, true);
                    self.elab.lctx.pop(id);
                }

                Stmt::Term((term, ty)) => {
                    result = self.elab.alloc.mkt_let(Atom::NULL, ty, term, result);
                }
            }
        }
        self.stmts.clear();

        for local in entry.params.copy_it().rev() {
            result    = self.elab.mk_binder(result,    local, false);
            result_ty = self.elab.mk_binder(result_ty, local, true);
        }
        entry.term = Some((result, result_ty));

        let name = &self.elab.strings[self.elab.env.symbol(entry.symbol_id).name];
        println!("{}:\n{}\n", name, self.elab.pp(result, 50));

        self.current_jp = None.into();
    }

    fn elab_do_block(&mut self, expr_id: ExprId, flags: ExprFlags, block: expr::Block, expected_ty: Option<Term<'out>>) -> Option<(Term<'out>, Term<'out>)> {
        let old_num_locals = self.locals.len();

        let jump_target = (block.is_do && flags.has_jump).then(|| {
            let jp = self.new_jp_with_locals(expected_ty.is_some());
            self.jump_targets.push(JumpTarget {
                jp,
                result_ty: expected_ty,
            });
            jp
        });

        for stmt_id in block.stmts.copy_it() {
            let stmt = self.parse.stmts[stmt_id];
            match stmt.kind {
                StmtKind::Error => (),

                StmtKind::Let(it) => {
                    let ty = if let Some(ty) = it.ty.to_option() {
                        let ty_expr = self.parse.exprs[ty];
                        if ty_expr.flags.has_loop || ty_expr.flags.has_jump || ty_expr.flags.has_assign {
                            // @todo: add local to prevent error cascade.
                            self.error(ty, ElabError::TempStr("type has loop/jump/assign"));
                            continue;
                        }
                        self.elab_expr_as_type(ty)?.0
                    }
                    else { self.new_ty_var().0 };

                    let val = if let Some(val) = it.val.to_option() {
                        self.elab_do_expr(val, Some(ty))?.0
                    }
                    else {
                        self.mkt_ax_uninit(ty)
                    };


                    // create local.
                    let name = it.name.value.to_option().unwrap_or(Atom::NULL);
                    let id = self.lctx.push(BinderKind::Explicit, name, ty, Some(val));
                    self.locals.push((name, id));

                    if self.current_jp.is_some() {
                        self.stmts.push(Stmt::Local(id));
                    }
                }

                StmtKind::Assign(lhs, rhs) => {
                    let lhs_expr = self.parse.exprs[lhs];
                    let ExprKind::Ident(ident) = lhs_expr.kind else {
                        self.error(lhs, ElabError::TempStr("invalid assign lhs"));
                        continue;
                    };

                    // find local.
                    let mut local = None;
                    for (index, (name, id)) in self.locals.copy_it().enumerate().rev() {
                        if name == ident.value {
                            local = Some((index, id));
                            break;
                        }
                    }
                    let Some((index, id)) = local else {
                        self.elab.error(ident.source, ElabError::UnresolvedName(
                            self.elab.alloc.alloc_str(&self.strings[ident.value])));
                        continue;
                    };

                    let ty = self.lctx.lookup(id).ty;
                    let Some((value, _)) = self.elab_do_expr(rhs, Some(ty)) else { continue };

                    // create new local.
                    let new_id = self.lctx.push(BinderKind::Explicit, ident.value, ty, Some(value));
                    self.locals[index].1 = new_id;

                    if self.current_jp.is_some() {
                        self.stmts.push(Stmt::Local(new_id));
                    }
                }

                StmtKind::Expr(it) => {
                    if let Some(term) = self.elab_do_expr(it, None) {
                        if self.current_jp.is_some() {
                            self.stmts.push(Stmt::Term(term));
                        }
                    }
                }
            }
        }

        self.locals.truncate(old_num_locals);

        if let Some(jp) = jump_target {
            let target = self.jump_targets.pop().unwrap();
            assert_eq!(target.jp, jp);

            let result_ty = target.result_ty.unwrap_or(Term::UNIT);

            if self.current_jp.is_some() {
                if let Some(result_ty) = target.result_ty {
                    // @todo: error to ax_sorry.
                    let jump_val = Term::UNIT_MK;
                    if !result_ty.syntax_eq(Term::UNIT) {
                        self.error(expr_id, ElabError::TempStr("block is unit, but unit is no good"));
                        return None;
                    }

                    self.end_jp(self.mk_jump(jp, Some(jump_val)));
                }
                else { self.end_jp(self.mk_jump(jp, None)); }
            }

            self.begin_jp(jp);
            let result_id = self.lctx.push(BinderKind::Explicit, Atom::NULL, result_ty, None);
            self.jps[jp].params.push(result_id);

            Some((self.alloc.mkt_local(result_id), result_ty))
        }
        else {
            // type validated by caller.
            Some((Term::UNIT_MK, Term::UNIT))
        }
    }

    fn elab_do_expr(&mut self, expr_id: ExprId, expected_ty: Option<Term<'out>>) -> Option<(Term<'out>, Term<'out>)> {
        let expr = self.parse.exprs[expr_id];

        // simple expr.
        let flags = expr.flags;
        if !flags.has_loop && !flags.has_jump && !flags.has_assign {
            return self.elab_expr_checking_type(expr_id, expected_ty);
        }

        let result = self.elab_do_expr_core(expr_id, expected_ty);

        // @todo: dedup (validate_type)
        #[cfg(debug_assertions)]
        if let Some((term, ty)) = result {
            let n = self.ivars.assignment_gen;
            let inferred = self.infer_type(term).unwrap();
            if !self.ensure_def_eq(ty, inferred) {
                eprintln!("---\nbug: elab_do_expr_core returned term\n{}\nwith type\n{}\nbut has type\n{}\n---",
                    self.pp(term, 80),
                    self.pp(ty, 80),
                    self.pp(inferred, 80));
                assert!(false);
            }
            assert_eq!(n, self.ivars.assignment_gen);
        }

        // ensure type.
        // @cleanup: dedup.
        if let (Some((_, ty)), Some(expected)) = (result, expected_ty) {
            if !self.ensure_def_eq(ty, expected) {
                let expected = self.instantiate_term_vars(expected);
                let ty       = self.instantiate_term_vars(ty);
                let expected = self.reduce_ex(expected, false);
                let ty       = self.reduce_ex(ty, false);
                self.elab.error(expr_id, {
                    let mut pp = TermPP::new(self.env, &self.strings, &self.lctx, self.alloc);
                    let expected = pp.pp_term(expected);
                    let found    = pp.pp_term(ty);
                    ElabError::TypeMismatch { expected, found }
                });
                return None;
            }
        }

        // expr info.
        if let Some((term, ty)) = result {
            debug_assert!(self.elab.elab.expr_infos[expr_id].is_none());
            self.elab.elab.expr_infos[expr_id] = Some(ExprInfo { term, ty });
        }

        return result;
    }

    fn elab_do_expr_core(&mut self, expr_id: ExprId, expected_ty: Option<Term<'out>>) -> Option<(Term<'out>, Term<'out>)> {
        let expr = self.parse.exprs[expr_id];
        Some(match expr.kind {
            ExprKind::Error => return None,

            ExprKind::Let(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do let"));
                return None;
            }

            ExprKind::Parens(it) => {
                return self.elab_do_expr(it, expected_ty);
            }

            ExprKind::Ref(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do ref"));
                return None;
            }

            ExprKind::Deref(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do deref"));
                return None;
            }


            ExprKind::Op1(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do op1"));
                return None;
            }

            ExprKind::Op2(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do op2"));
                return None;
            }

            ExprKind::Field(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do field"));
                return None;
            }

            ExprKind::Index(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do index"));
                return None;
            }

            ExprKind::Call(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do call"));
                return None;
            }

            ExprKind::Do(it) => {
                self.elab_do_block(expr_id, expr.flags, it, expected_ty)?
            }

            ExprKind::If(_) => {
                return self.elab_control_flow(expr_id, expected_ty);
            }

            ExprKind::While(_) => {
                return self.elab_control_flow(expr_id, expected_ty);
            }

            ExprKind::Loop(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do loop"));
                return None;
            }

            ExprKind::Break(it) => {
                // @todo: this is currently unreachable.
                // if we decide that function blocks are `do` blocks,
                // this can become an unwrap.
                let Some(target) = self.jump_targets.last().copied() else {
                    self.error(expr_id, ElabError::TempStr("no break target"));
                    return None;
                };

                if let Some(expected) = target.result_ty {
                    let value = if let Some(value) = it.value.to_option() {
                        self.elab_do_expr(value, Some(expected))?.0
                    }
                    else {
                        if !self.ensure_def_eq(expected, Term::UNIT) {
                            self.error(expr_id, ElabError::TempStr("break needs value"));
                        }
                        Term::UNIT_MK
                    };

                    if self.current_jp.is_some() {
                        self.end_jp(self.mk_jump(target.jp, Some(value)));
                    }
                }
                else {
                    if let Some(value) = it.value.to_option() {
                        self.elab_do_expr(value, Some(Term::UNIT))?;
                    }

                    if self.current_jp.is_some() {
                        self.end_jp(self.mk_jump(target.jp, None));
                    }
                }

                if let Some(expected) = expected_ty {
                    (self.mkt_ax_unreach(expected), expected)
                }
                else { (Term::UNIT_MK, Term::UNIT) }
            }

            ExprKind::Continue(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do continue"));
                return None;
            }

            ExprKind::ContinueElse(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do continue else"));
                return None;
            }

            ExprKind::Return(it) => {
                let expected = self.result_ty;
                let value = if let Some(value) = it.to_option() {
                    self.elab_do_expr(value, Some(expected))?.0
                }
                else {
                    if !self.ensure_def_eq(expected, Term::UNIT) {
                        self.error(expr_id, ElabError::TempStr("return needs value"));
                    }
                    Term::UNIT_MK
                };

                if self.current_jp.is_some() {
                    self.end_jp(value);
                }

                if let Some(expected) = expected_ty {
                    (self.mkt_ax_unreach(expected), expected)
                }
                else { (Term::UNIT_MK, Term::UNIT) }
            }

            ExprKind::TypeHint(_) => {
                self.error(expr_id, ElabError::TempStr("unimp do type hint"));
                return None;
            }

            // error.
            ExprKind::Sort(_) |
            ExprKind::Forall(_) |
            ExprKind::Lambda(_) |
            ExprKind::Eq(_, _) => {
                self.error(expr_id, ElabError::TempStr("not supported in do"));
                return None;
            }

            // expr flags are invalid.
            ExprKind::Hole |
            ExprKind::Ident(_) |
            ExprKind::DotIdent(_) |
            ExprKind::Path(_) |
            ExprKind::Levels(_) |
            ExprKind::Bool(_) |
            ExprKind::Number(_) |
            ExprKind::String(_) => unreachable!(),
        })
    }

    fn elab_control_flow(&mut self, expr_id: ExprId, expected_ty: Option<Term<'out>>) -> Option<(Term<'out>, Term<'out>)> {
        let expected = expected_ty.unwrap_or(Term::UNIT);

        let needs_value = expected_ty.is_some();
        let after_jp = self.new_jp_with_locals(needs_value);

        let mut all_then_unreachable = true;

        let mut curr = expr_id;
        loop {
            let expr = self.parse.exprs[curr];
            match expr.kind {
                ExprKind::If(it) => {
                    let (cond, _) = self.elab_do_expr(it.cond, Some(Term::BOOL))?;

                    let then_jp = self.new_jp_with_params_of(after_jp, false);

                    let (else_jp, else_arg) = if it.els.is_some() {
                        let else_jp = self.new_jp_with_params_of(after_jp, false);
                        (else_jp, None)
                    }
                    else {
                        if let Some(expected) = expected_ty {
                            if !self.ensure_def_eq(expected, Term::UNIT) {
                                self.error(curr, ElabError::TempStr("else is unit thing"));
                                return None;
                            }
                            (after_jp, Some(Term::UNIT_MK))
                        }
                        else { (after_jp, None) }
                    };

                    let curr_reachable = self.current_jp.is_some();
                    if curr_reachable {
                        self.end_jp(
                            self.alloc.mkt_apps(Term::ITE, &[
                                expected,
                                cond,
                                self.mk_jump(then_jp, None),
                                self.mk_jump(else_jp, else_arg)]));

                        self.begin_jp(then_jp);
                    }
                    else {
                        self.jps[then_jp].reachable = false;
                    }

                    let (then_val, _) = self.elab_do_expr(it.then, expected_ty)?;
                    let then_reachable = self.current_jp.is_some();
                    if then_reachable {
                        all_then_unreachable = false;
                        self.end_jp(self.mk_jump(after_jp, needs_value.then_some(then_val)));
                    }

                    if curr_reachable {
                        self.begin_jp(else_jp);
                    }
                    else {
                        self.jps[else_jp].reachable = false;
                    }

                    if let Some(els) = it.els.to_option() {
                        curr = els;
                    }
                    else { break }
                }

                ExprKind::While(_) => {
                    unimplemented!()
                }

                // else.
                _ => {
                    let (els_val, _) = self.elab_do_expr(curr, expected_ty)?;
                    let els_reachable = self.current_jp.is_some();
                    if els_reachable {
                        self.end_jp(self.mk_jump(after_jp, needs_value.then_some(els_val)));
                    }

                    if !els_reachable && all_then_unreachable {
                        self.jps[after_jp].reachable = false;
                    }
                    else {
                        self.begin_jp(after_jp);
                    }

                    break;
                }
            };
        }

        Some(if let Some(result_ty) = expected_ty {
            let result_id = self.lctx.push(BinderKind::Explicit, Atom::NULL, result_ty, None);
            self.jps[after_jp].params.push(result_id);

            (self.alloc.mkt_local(result_id), result_ty)
        }
        else { (Term::UNIT_MK, Term::UNIT) })
    }
}

impl<'me, 'e, 'c, 'out> core::ops::Deref for ElabDo<'me, 'e, 'c, 'out> {
    type Target = Elaborator<'e, 'c, 'out>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target { self.elab }
}

impl<'me, 'e, 'c, 'out> core::ops::DerefMut for ElabDo<'me, 'e, 'c, 'out> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target { self.elab }
}


