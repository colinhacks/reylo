create type zmood as enum ('a', 'b'); create macro a_lead(s) as s <> ''; create table t (id integer primary key, m zmood, code varchar check (a_lead(code)));
