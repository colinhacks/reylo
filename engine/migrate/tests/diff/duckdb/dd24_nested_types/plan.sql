alter table t add column tags varchar[];
alter table t add column meta struct(host varchar, port integer);
alter table t add column counts map(varchar, integer);
alter table t add column vec float[3];
alter table t add column u union(n integer, s varchar);
