create type mood as enum ('a', 'b'); create macro f(x) as x::mood <> 'b'; create table t (id integer primary key, m varchar check (f(m)));
