create type mood as enum ('a', 'b'); create macro a_lead(s) as s <> ''; create table t (id integer primary key, m mood, code varchar check (a_lead(code)));
