-- A unique index on a citext column under text_ops, whose equality tells 'A' from 'a': citext's
-- `=`, case-blind, matches both, so no key of type citext is pinned by it in this database.
create extension citext;
create table c (h citext not null, n text not null);
create unique index c_h on c (h text_ops);
insert into c values ('A', 'upper'), ('a', 'lower');
