-- A CHECK that keeps a string column to a list makes a bare read of it a union of literals:
-- validated, enforced, on that column alone, over text or varchar, or the column's domain's. A bare
-- parameter written whole to such a column is that union too; a cast one is its cast's type.
create domain mood as text check (value in ('ok', 'sad'));
create table accounts (id int primary key, status text not null check (status in ('active', 'banned')), tier varchar(10) check (tier in ('free', 'pro')), m mood, legacy text);
-- A row from before a NOT VALID check: the check says nothing of it.
insert into accounts values (99, 'active', null, null, 'zzz');
alter table accounts add constraint legacy_check check (legacy in ('a', 'b')) not valid;
create table pads (id int primary key, c char(3) check (c in ('a', 'b')));
