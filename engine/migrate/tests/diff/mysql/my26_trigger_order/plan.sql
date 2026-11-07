create trigger users_b before insert on users for each row follows users_a set new.n = new.n * 2;
