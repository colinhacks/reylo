alter table t add column s text;
alter table t set (toast.autovacuum_enabled=true);
