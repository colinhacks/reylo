-- What an INSERT's source says of its parameters: every row of a VALUES list writes to the target
-- columns, as the one-row form does, and so does the list of the SELECT that feeds an INSERT. A
-- constraint declared ON CONFLICT IGNORE, and a later ON CONFLICT clause, can leave no row to
-- return; a view takes a write through its INSTEAD OF trigger.
create table users (id integer primary key, name text not null, email text unique, nick text);
create table bar (name text not null, ready bool not null, n integer);
insert into bar values ('a', 1, 1), ('b', 0, 2), ('c', 'yes', 3);
create table foo (name text not null, meta text, k integer);
create table foo2 (name text not null, meta text, k integer);
create table ign (id integer primary key, k text unique on conflict ignore, v integer not null on conflict ignore);
create view named as select id, name from users;
create trigger named_insert instead of insert on named begin insert into users (name) values (new.name); end;
