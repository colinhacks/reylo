-- A WITH query's name may hold a quote, or a letter outside ASCII, and a table of that name may hold what the WITH query does not: a probe of a member keeps every WITH query it may read, so the name never falls through to the table.
create table `a``b` (n bigint not null);
create table plain_table (n bigint not null);
insert into `a``b` values (7);
insert into plain_table values (7);
