alter table t alter column b type bytea[] using case when num_nulls(b) = 1 then null else array[b] end;
alter table t alter column b drop not null;
alter table t alter column k type text[] using case when num_nulls(k) = 1 then null else array[k] end;
alter table t alter column n type numeric[] using case when num_nulls(n) = 1 then null else array[n] end;
alter table t alter column v type character varying(3)[] using case when num_nulls(v) = 1 then null else array[v] end;
