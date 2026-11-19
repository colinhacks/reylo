-- A CHECK that keeps a string column to a list makes a bare read of it a union of literals:
-- validated, enforced, on that column alone, over text or varchar, or the column's domain's. A bare
-- parameter written whole to such a column is that union too; a cast one is its cast's type.
create domain mood as text check (value in ('ok', 'sad'));
create table accounts (id int primary key, status text not null check (status in ('active', 'banned')), tier varchar(10) check (tier in ('free', 'pro')), m mood, legacy text);
-- A row from before a NOT VALID check: the check says nothing of it.
insert into accounts values (99, 'active', null, null, 'zzz');
alter table accounts add constraint legacy_check check (legacy in ('a', 'b')) not valid;
create table pads (id int primary key, c char(3) check (c in ('a', 'b')));
-- A CHECK that compares the column through a cast with a length compares what the cast cuts it
-- to: 'ab'::varchar(1) is 'a', so 'ab' passes `in ('a', 'b')` and is stored.
create table cut (id int primary key, code text check (code::varchar(1) in ('a', 'b')), codes text[] check (codes::varchar(1)[] <@ array['a', 'b']::varchar[]));
insert into cut (id, code) values (1, 'ab');
insert into cut (id, codes) values (2, '{ab}');
