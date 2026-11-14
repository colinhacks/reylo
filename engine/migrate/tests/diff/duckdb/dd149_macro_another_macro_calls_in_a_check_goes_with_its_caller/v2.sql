create table t (id integer primary key, qty integer check (qty > 0 and qty < 100));
