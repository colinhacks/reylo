create type mood as enum ('a');
create type pc as (m mood);
create table t (id integer primary key, m mood, c pc);
create function r() returns bigint language sql stable begin atomic select count(m) from t; end;
create function r2() returns bigint language sql stable begin atomic select count(c) from t; end;
