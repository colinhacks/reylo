create table logs (id int not null primary key, at datetime2(3) not null);
create table events (id bigint identity(1, 1) not null primary key, kind varchar(20) not null, at datetime2(7) not null default sysutcdatetime());
