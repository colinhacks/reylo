-- loses data: posts
drop table posts;
alter table users alter column age set data type bigint;
