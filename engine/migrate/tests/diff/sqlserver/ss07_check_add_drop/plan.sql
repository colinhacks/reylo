declare @reylo_ca4c6cc76f76 nvarchar(max) = (select top 1 quotename(name) from sys.check_constraints where parent_object_id = object_id(N'products') and definition = N'([price]>(0))');
if @reylo_ca4c6cc76f76 is null throw 50000, N'The constraint to drop on products, which SQL Server named itself, is not there.', 1;
exec (N'alter table products drop constraint ' + @reylo_ca4c6cc76f76);
alter table products add constraint stock_positive check ([stock]>=(0));
