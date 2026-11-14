create macro m(x) as x + 1; create table x (id integer primary key, d integer default m(1)); create table y (id integer primary key, xid integer references x (id), e integer);
