create table t (id int not null primary key, updated_at datetime2(3) null);
GO
create trigger t_touch on t after update as begin set nocount on; update t set updated_at = sysutcdatetime() from t join inserted i on i.id = t.id; end;
