-- A recursive WITH query's columns are a fixed point: its reads of itself are taken to be what
-- its first arm gives, then weakened to what the query gives under that, until nothing changes.
create table categories (id integer primary key, parent_id integer references categories (id), name text not null, note text);
