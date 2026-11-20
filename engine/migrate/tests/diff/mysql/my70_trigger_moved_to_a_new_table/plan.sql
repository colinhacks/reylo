-- loses data: old_t
drop table old_t;
create trigger tr before insert on new_t for each row set new.a = 1;
