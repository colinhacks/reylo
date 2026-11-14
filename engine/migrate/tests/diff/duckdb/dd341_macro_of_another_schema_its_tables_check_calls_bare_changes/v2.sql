create schema a; create macro a.valid(s) as length(s) > 0; create table a.t (id integer primary key, code varchar check (a.valid(code)));
