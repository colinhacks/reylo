-- users.full_name is renamed display_name, keeping its rows. If they are two columns, write instead:
--   alter table users drop column full_name;
--   alter table users add display_name nvarchar(100) not null;
exec sp_rename N'users.[full_name]', N'display_name', N'COLUMN';
