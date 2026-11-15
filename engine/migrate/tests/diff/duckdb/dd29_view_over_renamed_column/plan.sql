drop view v;
-- t.nick is renamed handle, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table t rename column nick to handle;
create view v as select handle from t;
