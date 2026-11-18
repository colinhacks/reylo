create table storage_modes (id integer primary key, payload text, doc jsonb, n integer);
alter table storage_modes alter column doc set storage external;
