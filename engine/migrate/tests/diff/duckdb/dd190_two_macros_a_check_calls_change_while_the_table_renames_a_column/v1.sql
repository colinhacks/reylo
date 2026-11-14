create macro m1(s) as s <> ''; create macro m2(s) as s <> 'x'; create table t (id integer primary key, code varchar check (m1(code) and m2(code)), note varchar);
