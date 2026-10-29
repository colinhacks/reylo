-- Set-returning functions in FROM: a strict one returns no row for a NULL argument, so a column
-- whose every row holds a value is never NULL, whatever its arguments, unless an outer join nulls
-- it. WITH ORDINALITY numbers the rows, and a LATERAL subquery that reads the items before it is
-- read by the rules alone.
create type mood as enum ('sad', 'ok', 'happy');
create table people (id bigint primary key, name text not null, tags text[] not null, prefs jsonb not null check (jsonb_typeof(prefs) = 'object'), list jsonb not null check (jsonb_typeof(list) = 'array'), csv text not null, n int not null);
create table visits (id bigint primary key, person_id bigint not null references people (id), at timestamptz not null, note text);
create function pair(p int, out a int, out b text) language sql as $$ select p, p::text $$;
