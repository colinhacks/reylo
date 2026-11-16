create table users (id int primary key, full_name varchar(200) not null, n int not null default 0, code text, added date, key full_name_idx (full_name)) comment = 'People';
