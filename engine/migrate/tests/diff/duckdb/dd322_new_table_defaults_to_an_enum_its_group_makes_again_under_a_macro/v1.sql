create type zmood as enum ('a', 'b'); create macro a_lead(s) as s <> ''; create table t (id integer primary key, code varchar check (a_lead(code) and code::zmood <> 'b'));
