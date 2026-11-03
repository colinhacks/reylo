alter table hot reset (autovacuum_enabled);
alter table hot set (autovacuum_vacuum_scale_factor=0.05, fillfactor=90);
