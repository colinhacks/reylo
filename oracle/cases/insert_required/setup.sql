-- An INSERT that leaves out a column each row must write, or writes DEFAULT to one, and the
-- columns something else fills: a default, a domain's default, a trigger, an identity, a
-- generation expression.
create table req (id serial primary key, name text not null, email text not null, note text);
create table src (id int, name text);
create domain dflt_text as text default 'x';
create table with_domain (id int primary key, label dflt_text not null);
create domain nn_text as text not null;
create table with_nn_domain (id int primary key, code nn_text);
create table with_trigger (id int primary key, slug text not null);
create function fill_slug() returns trigger language plpgsql as $$ begin new.slug := 'auto'; return new; end $$;
create trigger fill_slug before insert on with_trigger for each row execute function fill_slug();
create table generated (id int primary key, twice int not null generated always as (id * 2) stored);
create table ident (id int generated always as identity primary key, name text not null);
create table parent_t (id int primary key, parent_name text not null);
create table child_t (child_name text not null) inherits (parent_t);
create type id_name as (id int, name text);
