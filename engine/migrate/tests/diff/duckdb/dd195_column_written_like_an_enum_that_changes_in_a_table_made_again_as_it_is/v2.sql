create type mood as enum ('a', 'b', 'c'); create macro valid(s) as length(s) > 0; create table t (id integer primary key, code varchar check (valid(code)), status enum ('a', 'b'));
