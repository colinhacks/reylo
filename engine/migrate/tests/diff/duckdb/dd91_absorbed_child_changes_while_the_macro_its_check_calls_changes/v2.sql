create macro ok(x) as x >= 0; create table p (id integer primary key, n integer check (n > 0)); create table c (id integer, p_id integer references p (id), v integer check (ok(v)), note varchar);
