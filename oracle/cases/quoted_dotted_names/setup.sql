-- A dot inside a quoted name is part of it: "a.b" is one table, found through the search path, and a.b is the table b of the schema a.
create schema a;
create table a.b (id int primary key, v text not null);
create table "a.b" (id int primary key, v text);
create function skip_row() returns trigger language plpgsql as $$ begin return null; end $$;
create trigger skip before insert on "a.b" for each row when (new.v = 'skip') execute function skip_row();
insert into "a.b" values (1, null);
insert into a.b values (1, 'one');
