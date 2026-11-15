create type mood as enum ('sad', 'ok'); create table t (id bigint primary key, m mood);
