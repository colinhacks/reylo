-- An aggregate with no GROUP BY returns one row even over no rows, and a column outside an
-- aggregate's arguments is NULL in that row, whatever its table declares and the WHERE tests.
-- And `x NOT IN (SELECT …)` is true of a NULL `x` where the subquery returns no row.
create table e (id integer primary key, name text not null, a integer not null);
create table opt (id integer primary key, x integer, z integer not null) strict;
create table none (id integer primary key, v integer not null) strict;
