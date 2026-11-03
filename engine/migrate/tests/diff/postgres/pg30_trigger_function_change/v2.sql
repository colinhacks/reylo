create table users (id integer primary key, name text not null, updated_at timestamptz);
create function stamp() returns trigger language plpgsql as $$ begin new.updated_at = statement_timestamp(); return new; end $$;
create trigger users_touch before update on users for each row execute function stamp();
