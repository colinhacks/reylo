create table items (id int not null primary key, qty int not null constraint df_items_qty default 1, note nvarchar(20) null default N'n/a');
