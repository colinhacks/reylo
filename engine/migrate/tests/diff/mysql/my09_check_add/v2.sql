create table users (id int primary key, full_name varchar(100) not null, n int not null, code text, constraint n_pos check (n > 0));
