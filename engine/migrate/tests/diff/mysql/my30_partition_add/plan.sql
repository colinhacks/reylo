alter table hits add column m int;
alter table hits partition by hash (id) partitions 4;
