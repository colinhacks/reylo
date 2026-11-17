-- loses data: a
declare @reylo_f714e605bd15 nvarchar(max) = (select top 1 quotename(f.name) from sys.foreign_keys f where f.parent_object_id = object_id(N'b') and f.referenced_object_id = object_id(N'a') and (select string_agg(c.name, N',') within group (order by fc.constraint_column_id) from sys.foreign_key_columns fc join sys.columns c on c.object_id = fc.parent_object_id and c.column_id = fc.parent_column_id where fc.constraint_object_id = f.object_id) = N'a_id');
exec (N'alter table b drop constraint ' + @reylo_f714e605bd15);
drop table a;
