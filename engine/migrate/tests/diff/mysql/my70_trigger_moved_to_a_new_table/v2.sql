create table new_t (id int primary key, a int);
create trigger tr before insert on new_t for each row set new.a = 1;
