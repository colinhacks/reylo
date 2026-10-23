-- Inside a JSON document Postgres writes a value through its type's cast to JSON where it has one
-- (an hstore's, in json_hstore), and otherwise as a number only for the integers, the floats and
-- numeric: an oid, and the lo domain over it, is its text, and so is every extension type here.
create extension lo;
create extension cube;
create extension earthdistance;
create extension seg;
create extension isn;
create extension intarray;
create table blobs (id bigint primary key, l lo not null, o oid not null);
create table shapes (id bigint primary key, c cube not null, e earth, s seg, i isbn, q query_int);
insert into blobs values (1, 4294967295, 0), (2, 7, 16384);
insert into shapes values (1, '(1,2)', ll_to_earth(1, 2), '1 .. 2', '0-306-40615-2', '1&2'), (2, '(0)', null, null, null, null);
