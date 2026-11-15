create table x (id integer primary key, d integer); create table y (id integer primary key, xid integer references x (id), e integer);
