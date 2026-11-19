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
-- Volatility that reaches a body another way: a function it calls whose own body is volatile, a
-- view that calls a volatile function, an argument's default, a set-returning function it reads,
-- an operator whose function is volatile, and a sample of a table, drawn anew for each call.
create function turn_outer() returns int language sql stable as $$ select turn() $$;
create view turns_view as select (nextval('turns') % 2 + 1)::int as r;
create function turn_view() returns int language sql stable as $$ select r from turns_view $$;
create function turn_default(x int default (nextval('turns') % 2 + 1)::int) returns int language sql stable as $$ select x $$;
create function turns_set() returns setof int language sql stable as $$ select (nextval('turns') % 2 + 1)::int $$;
create function turn_from() returns int language sql stable as $$ select v from turns_set() v limit 1 $$;
create function turn_op(int) returns int language sql stable as $$ select $1 * 0 + (nextval('turns') % 2 + 1)::int $$;
create operator ### (function = turn_op, rightarg = int);
create function turn_operator() returns int language sql stable as $$ select ### 1 $$;
create function sampled() returns int language sql stable as $$ select id from rk tablesample bernoulli (50) order by id limit 1 $$;
-- A function in another language is taken at its label.
create function turn_plpgsql() returns int language plpgsql stable as $$ begin return 1; end $$;
-- A body that calls a function whose own body calls nothing volatile keeps its label.
create function first_outer() returns int language sql stable as $$ select first_id() $$;
