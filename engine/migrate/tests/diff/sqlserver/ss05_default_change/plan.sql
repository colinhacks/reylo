alter table items drop constraint df_items_note;
alter table items add default (N'n/a') for note;
declare @reylo_13af63d88e28 nvarchar(max) = (select top 1 quotename(d.name) from sys.default_constraints d join sys.columns c on c.object_id = d.parent_object_id and c.column_id = d.parent_column_id where d.parent_object_id = object_id(N'items') and c.name = N'qty');
exec (N'alter table items drop constraint ' + @reylo_13af63d88e28);
alter table items add constraint df_items_qty default ((1)) for qty;
