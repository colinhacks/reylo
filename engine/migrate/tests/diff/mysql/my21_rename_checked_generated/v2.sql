create table users (id int primary key, total int not null, n2 int generated always as (total * 2) virtual, constraint n_pos check (total > 0));
