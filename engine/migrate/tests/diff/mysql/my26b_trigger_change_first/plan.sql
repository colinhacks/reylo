drop trigger users_b;
drop trigger users_a;
create trigger users_a before insert on users for each row set new.n = new.n + 5;
create trigger users_b before insert on users for each row follows users_a set new.n = new.n * 2;
