use crate::ast::{ItemId, ItemKind, item, IdentOrPath};

use super::*;

impl<'me, 'c, 'out> Elaborator<'me, 'c, 'out> {
    pub fn elab_item(&mut self, item_id: ItemId) -> Option<()> {
        let item = &self.parse.items[item_id];

        let info = match &item.kind {
            ItemKind::Error => return None,

            ItemKind::Axiom(it) => {
                let symbol = self.elab_axiom(item_id, it)?;
                ItemInfo::Symbol(symbol)
            }

            ItemKind::Def(it) => {
                let symbol = self.elab_def(item_id, it)?;
                ItemInfo::Symbol(symbol)
            }

            ItemKind::Reduce(it) => {
                spall::trace_scope!("kibi/elab/reduce");

                let (term, _) = self.elab_expr(*it);
                let r = self.reduce(term);
                ItemInfo::Reduce(r)
            }

            ItemKind::Print(it) => {
                spall::trace_scope!("kibi/elab/print");

                let path = match it {
                    IdentOrPath::Ident(ident) => core::slice::from_ref(ident),
                    IdentOrPath::Path(path) => path.parts,
                };

                let symbol_id = self.elab_path(path)?;
                ItemInfo::Print(symbol_id)
            }

            ItemKind::Inductive(it) => {
                spall::trace_scope!("kibi/elab/inductive"; "{}",
                    &self.strings[it.name.value]);

                let symbol = self.elab_inductive(item_id, it)?;
                ItemInfo::Symbol(symbol)
            }

            ItemKind::Trait(it) => {
                match it {
                    item::Trait::Inductive(ind) => {
                        spall::trace_scope!("kibi/elab/trait-ind",
                            &self.strings[ind.name.value]);

                        let symbol = self.elab_inductive(item_id, &ind)?;

                        self.traits.new_trait(symbol);

                        ItemInfo::Symbol(symbol)
                    }
                }
            }

            ItemKind::Impl(it) => {
                spall::trace_scope!("kibi/elab/impl");

                let symbol_id = self.env.reserve_id();

                let (ty, _) = self.elab_def_core(symbol_id, item_id,
                    it.levels, it.params, it.ty.some(), it.value)?;

                let trayt = ty.forall_ret().0.app_fun().0;
                let mut is_trait = false;
                if let Some(g) = trayt.try_global() {
                    if self.traits.is_trait(g.id) {
                        is_trait = true;

                        let impls = self.traits.impls(g.id);
                        // @speed: arena.
                        let name = self.strings.insert(&format!("impl_{}", impls.len()));
                        self.env.attach_id(symbol_id, g.id, name)?;
                        self.traits.add_impl(g.id, symbol_id);
                    }
                }
                if !is_trait {
                    // @todo: better source, type.
                    self.error(item_id, ElabError::ImplTypeIsNotTrait);
                    return None;
                }

                // @todo: better source.
                let _ = self.check_no_unassigned_variables(item_id.into())?;

                // @todo: item info.
                return Some(());
            }
        };

        assert_eq!(self.aux_defs.len(), 0);

        debug_assert!(self.elab.item_infos[item_id].is_none());
        self.elab.item_infos[item_id] = Some(info);

        Some(())
    }
}

