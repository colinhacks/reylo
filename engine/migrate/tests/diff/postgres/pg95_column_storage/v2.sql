create table storage_modes (id integer primary key, payload text, doc jsonb, n integer);
alter table storage_modes alter column payload set storage external;
alter table storage_modes alter column payload set compression pglz;
alter table storage_modes alter column n set statistics 500;
