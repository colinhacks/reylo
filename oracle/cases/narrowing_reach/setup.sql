-- What a test narrows beyond a plain column of a SELECT's WHERE: the columns of a `*`, the columns
-- an inner join's own ON tests, a write's WHERE for its RETURNING, a WITH query read twice, and a
-- recursive WITH query that reads `*` or has SEARCH or CYCLE.
create table users (id int8 primary key, name text not null, nickname text);
create table orders (id int8 primary key, note text, total numeric, user_id int8);
create table comments (id int8 primary key, parent_id int8, author_id int8, body text not null);
create table staff (id int8 primary key, manager_id int8, name text);
insert into users values (1, 'a', null), (2, 'b', 'bee');
insert into orders values (1, null, null, null), (2, 'n', 5, 1), (3, 'm', null, 99);
insert into comments values (1, null, null, 'x'), (2, 1, 1, 'y'), (3, 2, 99, 'z'), (4, 1, null, 'w');
insert into staff values (1, null, 'boss'), (2, 1, null), (3, 2, 'c');
-- A write whose row a trigger, a generation expression or a rule may change after its WHERE.
create table gen (id int8 primary key, a int8, g int8 generated always as (nullif(a, 0)) stored);
insert into gen values (1, 1), (2, 0), (3, null);
create table trig (id int8 primary key, v text);
insert into trig values (1, 'a'), (2, null);
create function trig_null() returns trigger language plpgsql as $$ begin new.v := null; return new; end $$;
create trigger trig_null before update on trig for each row execute function trig_null();
create table ruled (id int8 primary key, v text);
create table ruled_log (id int8, v text);
insert into ruled values (1, 'a');
create rule ruled_r as on update to ruled do instead update ruled_log set v = null where ruled_log.id = old.id returning ruled_log.*;
insert into ruled_log values (1, 'a');
create table parts (id int8, v text) partition by list (id);
create table parts_1 partition of parts for values in (1);
create table parts_2 partition of parts for values in (2);
insert into parts values (1, 'a'), (2, 'b');
create function parts_null() returns trigger language plpgsql as $$ begin new.v := null; return new; end $$;
create trigger parts_null before insert on parts_2 for each row execute function parts_null();
