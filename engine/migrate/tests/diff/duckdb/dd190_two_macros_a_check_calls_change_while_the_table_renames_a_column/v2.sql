create macro m1(s) as length(s) > 0; create macro m2(s) as s not in ('x'); create table t (id integer primary key, code varchar check (m1(code) and m2(code)), remark varchar);
