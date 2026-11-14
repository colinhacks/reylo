create schema a; create macro a.valid(s) as s <> ''; create table a.t (id integer primary key, code varchar check (a.valid(code)));
