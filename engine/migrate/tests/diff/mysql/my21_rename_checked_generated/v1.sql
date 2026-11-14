create table users (id int primary key, n int not null, n2 int generated always as (n * 2) virtual, constraint n_pos check (n > 0));
