alter table p alter column c type json[] using case when num_nulls(c) = 1 then null else array[c] end;
alter table pp alter column n type integer[] using case when num_nulls(n) = 1 then null else array[n] end;
