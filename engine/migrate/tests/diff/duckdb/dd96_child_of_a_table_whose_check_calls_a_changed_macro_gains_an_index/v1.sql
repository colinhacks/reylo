create macro ok(x) as x > 0; create table p (id integer primary key, v integer check (ok(v))); create table c (id integer, p_id integer references p (id), note varchar);
