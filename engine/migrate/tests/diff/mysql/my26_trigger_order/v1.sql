create table users (id int primary key, n int not null); create trigger users_a before insert on users for each row set new.n = new.n + 1;
