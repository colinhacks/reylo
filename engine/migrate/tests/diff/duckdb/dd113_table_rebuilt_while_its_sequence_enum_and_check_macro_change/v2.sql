create sequence s increment by 2; create type mood as enum ('a', 'b', 'c'); create macro valid(c) as length(c) > 0;
     create table t (id bigint primary key default nextval('s'), m mood, code varchar check (valid(code)), n integer check (n > 0));
     create index t_m on t (m);
     comment on column t.code is 'the code';
     create table c (id integer primary key, t_id bigint references t (id));
