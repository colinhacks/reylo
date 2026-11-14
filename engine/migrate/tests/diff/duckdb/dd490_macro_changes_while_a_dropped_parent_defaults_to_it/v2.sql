create macro m(x) as x + 2; create table y (id integer primary key, xid integer, e integer default m(2));
