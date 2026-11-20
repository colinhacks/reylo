alter table k reset (toast.autovacuum_enabled);
alter table t set (toast.autovacuum_vacuum_cost_delay=23);
