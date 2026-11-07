-- A whole row written `t.*` inside JSON the statement builds, an array with a NULL element inside
-- it, the floats and decimals JSON has no number for, and two result columns of one name.
create table teams (id bigint primary key, name text not null, score float8);
create table members (id bigint primary key, team_id bigint references teams (id), name text not null, rating numeric);
-- An alias that renames a table's columns leaves its whole row's names, which are its type's; a
-- subquery's row takes the alias's.
create function whole_teams() returns setof teams language sql stable as $$ select * from teams $$;
create function team_pairs() returns table (id bigint, name text) language sql stable as $$ select id, name from teams $$;
create view team_names as select id, name from teams;
