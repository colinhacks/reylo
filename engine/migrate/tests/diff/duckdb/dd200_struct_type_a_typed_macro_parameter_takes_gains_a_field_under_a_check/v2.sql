create type rec as struct(n integer, m integer); create macro positive(r rec) as r.n > 0; create table t (id integer primary key, r rec, check (positive(r)));
