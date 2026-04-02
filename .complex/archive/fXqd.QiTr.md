Replace all dot-based hierarchy logic with parent-field traversal:
- model.rs: children() uses n.parent == Some(id), roots() uses n.parent.is_none(), effective_tags() walks parent chain. Add ancestors(), descendants(), is_descendant_of().
- main.rs: ancestor_blocker() uses graph.ancestors(). Simplify cmd_move() to just update parent field. Fix comment file rename bug first.
- db.rs: add parent column, replace LIKE SQL with recursive CTE or ancestor table.
- Display: use node.id directly (no rfind('.') leaf extraction).
