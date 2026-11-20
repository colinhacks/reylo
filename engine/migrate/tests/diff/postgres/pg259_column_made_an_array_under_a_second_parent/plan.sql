alter table b alter column c type e[] using case when num_nulls(c) = 1 then null else array[c] end;
