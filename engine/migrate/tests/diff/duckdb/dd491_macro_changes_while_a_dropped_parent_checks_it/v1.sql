create macro m(x) as x > 0; create table x (id integer primary key, d integer check (m(d))); create table y (id integer primary key, xid integer references x (id), e integer check (m(e)));
