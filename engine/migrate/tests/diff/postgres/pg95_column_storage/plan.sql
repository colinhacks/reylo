alter table storage_modes alter column doc set storage extended;
alter table storage_modes alter column payload set compression pglz;
alter table storage_modes alter column n set statistics 500;
alter table storage_modes alter column payload set storage external;
