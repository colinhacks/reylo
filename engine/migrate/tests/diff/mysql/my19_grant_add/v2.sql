create table users (id int primary key, full_name varchar(100) not null, n int not null, code text);
grant select on users to 'reader'@'%';
