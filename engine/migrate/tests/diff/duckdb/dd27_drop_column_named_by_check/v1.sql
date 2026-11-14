create table t (id bigint primary key, lo integer, hi integer, check (lo < hi));
