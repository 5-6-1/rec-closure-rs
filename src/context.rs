//! Lexical context for closures found inside blocks and control-flow scopes.

use syn::{
    Expr, ExprClosure, ItemFn, Pat,
    visit_mut::{self, VisitMut},
};

use crate::{
    Ctx, expand_stmts,
    scope::{condition_names, pat_names},
};

impl Ctx {
    fn mask_pattern(&mut self, pat: &Pat) {
        let names = pat_names(pat);
        self.items.retain(|id| !names.contains(id));
    }

    pub(crate) fn expand_body(&mut self, closure: &ExprClosure, body: &mut Expr) {
        let saved = self.items.clone();
        for input in &closure.inputs {
            self.mask_pattern(input);
        }
        self.visit_expr_mut(body);
        self.items = saved;
    }
}

impl VisitMut for Ctx {
    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        // expand_stmts recursively visits original statements once. Visiting
        // the generated statements again would leak synthetic item names.
        expand_stmts(&mut block.stmts, self);
    }

    fn visit_item_fn_mut(&mut self, node: &mut ItemFn) {
        let items = self.items.clone();
        let generics = self.generics.clone();
        self.generics.extend(crate::signature::generic_names(&node.sig.generics));
        for input in &node.sig.inputs {
            if let syn::FnArg::Typed(input) = input {
                self.mask_pattern(&input.pat);
            }
        }
        visit_mut::visit_item_fn_mut(self, node);
        self.items = items;
        self.generics = generics;
    }

    fn visit_expr_closure_mut(&mut self, node: &mut ExprClosure) {
        let saved = self.items.clone();
        for input in &node.inputs {
            self.mask_pattern(input);
        }
        self.visit_expr_mut(&mut node.body);
        self.items = saved;
    }

    fn visit_expr_for_loop_mut(&mut self, node: &mut syn::ExprForLoop) {
        self.visit_expr_mut(&mut node.expr);
        let saved = self.items.clone();
        self.mask_pattern(&node.pat);
        self.visit_block_mut(&mut node.body);
        self.items = saved;
    }

    fn visit_expr_binary_mut(&mut self, node: &mut syn::ExprBinary) {
        self.visit_expr_mut(&mut node.left);
        let saved = self.items.clone();
        if matches!(node.op, syn::BinOp::And(_)) {
            let names = condition_names(&node.left);
            self.items.retain(|id| !names.contains(id));
        }
        self.visit_expr_mut(&mut node.right);
        self.items = saved;
    }

    fn visit_expr_if_mut(&mut self, node: &mut syn::ExprIf) {
        self.visit_expr_mut(&mut node.cond);
        let saved = self.items.clone();
        let names = condition_names(&node.cond);
        self.items.retain(|id| !names.contains(id));
        self.visit_block_mut(&mut node.then_branch);
        self.items = saved;
        if let Some((_, els)) = &mut node.else_branch {
            self.visit_expr_mut(els);
        }
    }

    fn visit_expr_while_mut(&mut self, node: &mut syn::ExprWhile) {
        self.visit_expr_mut(&mut node.cond);
        let saved = self.items.clone();
        let names = condition_names(&node.cond);
        self.items.retain(|id| !names.contains(id));
        self.visit_block_mut(&mut node.body);
        self.items = saved;
    }

    fn visit_expr_match_mut(&mut self, node: &mut syn::ExprMatch) {
        self.visit_expr_mut(&mut node.expr);
        for arm in &mut node.arms {
            let saved = self.items.clone();
            self.mask_pattern(&arm.pat);
            self.visit_pat_mut(&mut arm.pat);
            self.visit_expr_mut(&mut arm.body);
            self.items = saved;
        }
    }
}
