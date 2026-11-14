create macro is_pos(x) as x >= 1; create macro ok_qty(x) as is_pos(x) and x < 100; create table t (id integer primary key, qty integer check (ok_qty(qty)));
