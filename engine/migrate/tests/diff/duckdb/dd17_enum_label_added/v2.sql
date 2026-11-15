create type mood as enum ('sad', 'ok', 'glad'); create table t (id bigint primary key, m mood);
