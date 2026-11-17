alter table prices alter column amount decimal(12, 2) not null;
alter table prices add note nvarchar(100) null;
