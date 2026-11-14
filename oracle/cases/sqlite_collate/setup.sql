-- A COLLATE names how a value compares and leaves the value as it is: a parameter under one
-- keeps the type of what it is compared with. The key is another matter: `b` is unique under
-- BINARY, which tells 'a' from 'A', and a comparison under NOCASE meets both rows.
create table c (id integer primary key, b text unique, n text unique collate nocase, name text not null);
insert into c values (1, 'a', 'x', 'p'), (2, 'A', 'y', 'P');
-- A comparison uses the left operand's column's collation, else the right's, and a subquery's
-- column keeps the collation of what it selects.
create table other (id integer primary key, n text collate nocase, b text);
insert into other values (1, 'a', 'a');
