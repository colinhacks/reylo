-- A key compared with a call of a `language sql` function labelled `stable` whose body calls
-- something volatile: a sequential scan calls it for each row, so the key matches several rows.
-- With the index scans off, every scan here is a sequential one.
set enable_indexscan = off;
set enable_indexonlyscan = off;
set enable_bitmapscan = off;
create table rk (id int primary key);
insert into rk values (1), (2), (3);
create sequence turns;
create function turn() returns int language sql stable as $$ select (nextval('turns') % 2 + 1)::int $$;
create function coin() returns int language sql stable as $$ select floor(random() * 1000)::int % 2 + 1 $$;
-- A body that calls nothing volatile keeps its label.
create function first_id() returns int language sql stable as $$ select 1 $$;
