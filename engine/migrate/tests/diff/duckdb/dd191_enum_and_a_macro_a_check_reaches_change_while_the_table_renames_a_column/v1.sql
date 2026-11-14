create type mood as enum ('a', 'b'); create macro valid(s) as s <> ''; create table t (id integer primary key, code varchar check (valid(code) and code::mood is not null), note varchar);
