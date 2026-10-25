-- A function whose rows have a column `sum` by an OUT parameter: no row type of the catalog has
-- that attribute, and the statement never writes the name. `sum(r)` over its row matches no `sum`
-- and is `r.sum`, a value per row (ParseFuncOrColumn's projection), no aggregate.
create table t (id int primary key, n int not null);
insert into t values (1, 1), (2, 2), (3, 3);
create function f(x int, out sum int, out other int) returns setof record language sql stable as $$ select x, x union all select x + 1, x $$;
create function g() returns table (sum int) language sql stable as $$ select 1 union all select 2 $$;
