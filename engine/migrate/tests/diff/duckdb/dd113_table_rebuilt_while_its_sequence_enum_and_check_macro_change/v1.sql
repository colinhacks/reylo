create sequence s; create type mood as enum ('a', 'b'); create macro valid(c) as c <> '';
     create table t (id bigint primary key default nextval('s'), m mood, code varchar check (valid(code)));
     create table c (id integer primary key, t_id bigint references t (id));
