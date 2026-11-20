declare @reylo_d5882adb8186 nvarchar(max) = (select top 1 quotename(f.name) from sys.foreign_keys f where f.parent_object_id = object_id(N'people') and f.referenced_object_id = object_id(N'teams') and (select string_agg(c.name, N',') within group (order by fc.constraint_column_id) from sys.foreign_key_columns fc join sys.columns c on c.object_id = fc.parent_object_id and c.column_id = fc.parent_column_id where fc.constraint_object_id = f.object_id) = N'team_id');
if @reylo_d5882adb8186 is null throw 50000, N'The constraint to drop on people, which SQL Server named itself, is not there.', 1;
exec (N'alter table people drop constraint ' + @reylo_d5882adb8186);
alter table people add constraint people_boss foreign key (boss_id) references people (id);
