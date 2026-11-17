drop index tags_name on tags;
alter table tags alter column name nvarchar(60) not null;
create unique nonclustered index tags_name on tags (name);
