create table t (s text) with (fillfactor = 80);
create table k (s text) with (toast.autovacuum_enabled = false);
