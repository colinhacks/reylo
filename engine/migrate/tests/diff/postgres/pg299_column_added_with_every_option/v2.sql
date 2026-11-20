create table t (id int, name text collate "C" not null default 'x' check (name <> ''), stamp timestamptz default now());
create materialized view mv as select id from t;
comment on materialized view mv is 'ids';
