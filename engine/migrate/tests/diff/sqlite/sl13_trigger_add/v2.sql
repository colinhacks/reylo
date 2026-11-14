create table users (id integer primary key, full_name text not null, updated_at text);
create trigger users_touch after update of full_name on users begin update users set updated_at = datetime('now') where id = new.id; end;
