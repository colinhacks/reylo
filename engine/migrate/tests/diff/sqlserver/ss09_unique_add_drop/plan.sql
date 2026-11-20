declare @reylo_b1b001c6f3b6 nvarchar(max) = (select top 1 quotename(k.name) from sys.key_constraints k where k.parent_object_id = object_id(N'accounts') and k.type = 'UQ' and (select string_agg(c.name, N',') within group (order by ic.key_ordinal) from sys.index_columns ic join sys.columns c on c.object_id = ic.object_id and c.column_id = ic.column_id where ic.object_id = k.parent_object_id and ic.index_id = k.unique_index_id and ic.key_ordinal > 0) = N'email');
if @reylo_b1b001c6f3b6 is null throw 50000, N'The constraint to drop on accounts, which SQL Server named itself, is not there.', 1;
exec (N'alter table accounts drop constraint ' + @reylo_b1b001c6f3b6);
alter table accounts add constraint accounts_handle unique nonclustered (handle);
