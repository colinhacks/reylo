alter table only c alter column c drop default;
alter table p alter column c type text[] using case when num_nulls(c) = 1 then null else array[c] end;
alter table c alter column c set default '{}'::text[];
