create macro m(x) as x > -1; create table y (id integer primary key, xid integer, e integer check (m(e)));
