create table old_t (id int primary key, a int);
create table new_t (id int primary key, a int);
create trigger tr before insert on old_t for each row set new.a = 1;
