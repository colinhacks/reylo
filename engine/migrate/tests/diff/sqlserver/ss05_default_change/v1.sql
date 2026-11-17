create table items (id int not null primary key, qty int not null default 0, note nvarchar(20) null constraint df_items_note default N'none');
