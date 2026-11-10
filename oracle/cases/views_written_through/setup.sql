-- RETURNING of an INSERT, an UPDATE or a MERGE gives the row as it was written. Through a view
-- that row need not pass the view's WHERE, an INSTEAD OF trigger returns a row of its own making,
-- and a rule's own RETURNING gives the row in the statement's place: a column read from such a
-- target is nullable and unnarrowed. A DELETE returns a row the view held, and a read is a read.
create table posts (id int primary key, title text not null, published_at timestamptz, status text not null default 'draft' check (status in ('draft', 'live')));
create view published as select id, title, published_at, status from posts where published_at is not null;
create table notes (id int primary key, body text not null, kind text not null default 'a' check (kind in ('a', 'b')), n int not null default 0);
create view note_list as select id, body, kind, n from notes;
create function note_list_insert() returns trigger language plpgsql as $$ begin insert into notes (id, body) values (new.id, coalesce(new.body, 'none')); return new; end $$;
create trigger note_list_insert instead of insert on note_list for each row execute function note_list_insert();
create function note_list_update() returns trigger language plpgsql as $$ begin update notes set body = coalesce(new.body, 'none') where id = old.id; new.kind := 'zzz'; return new; end $$;
create trigger note_list_update instead of update on note_list for each row execute function note_list_update();
create table inbox (id int primary key, name text not null);
create table archive (id int primary key, name text);
create rule inbox_insert as on insert to inbox do instead insert into archive values (new.id, null) returning archive.id, archive.name;
