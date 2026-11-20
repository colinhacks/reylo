-- loses data: t.old_col
alter table t drop column old_col;
alter table t add column new_col boolean;
