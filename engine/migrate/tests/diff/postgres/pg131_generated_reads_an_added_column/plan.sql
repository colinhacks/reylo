alter table t add column z_qty integer default 1;
alter table t add column a_total integer generated always as (z_qty * 2) stored;
