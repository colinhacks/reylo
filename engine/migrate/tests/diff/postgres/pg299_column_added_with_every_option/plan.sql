-- file 1 of 2
alter table t add column name text collate pg_catalog."C" default 'x' not null;
alter table t add column stamp timestamp with time zone default now();
alter table t add constraint t_name_check check (name <> '') not valid;
create materialized view mv as select id from t;
comment on materialized view mv is 'ids';
-- file 2 of 2
alter table t validate constraint t_name_check;
