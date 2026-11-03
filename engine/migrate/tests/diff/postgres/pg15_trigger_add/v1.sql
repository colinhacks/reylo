create table users (id integer primary key, full_name text not null, updated_at timestamptz);
create function touch() returns trigger language plpgsql as $$ begin new.updated_at := now(); return new; end $$;
