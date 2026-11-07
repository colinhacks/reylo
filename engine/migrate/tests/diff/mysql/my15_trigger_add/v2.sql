create table users (id int primary key, full_name varchar(100) not null, updated_at datetime null);
create trigger users_touch before update on users for each row set new.updated_at = now();
