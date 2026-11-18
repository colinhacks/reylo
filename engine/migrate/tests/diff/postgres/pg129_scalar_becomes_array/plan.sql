alter table t alter column b type bytea[] using case when b is null then null else array[b] end;
alter table t alter column b drop not null;
alter table t alter column k type text[] using case when k is null then null else array[k] end;
alter table t alter column n type numeric[] using case when n is null then null else array[n] end;
alter table t alter column v type character varying(3)[] using case when v is null then null else array[v] end;
