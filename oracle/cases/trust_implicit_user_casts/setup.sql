-- Implicit casts of a user's from a type of theirs to a built-in type, by a function that may answer
-- NULL, as PostGIS has one from geometry to text: a built-in function or operator over a value of
-- that type runs the cast on its way to the built-in, so the built-in's rule proves nothing of it.
create table t (id int primary key, note text, s text not null, v varchar(20));
-- A composite type whose cast to text is NULL for anything.
create type boxed as (v int);
create function boxed_text(boxed) returns text language sql as 'select null::text';
create cast (boxed as text) with function boxed_text(boxed) as implicit;
create table tb (id int primary key, b boxed not null, v varchar(20));
insert into tb values (1, row(1), null);
-- An enum whose cast to text is NULL for anything.
create type mood as enum ('happy', 'sad');
create function mood_text(mood) returns text language sql as 'select null::text';
create cast (mood as text) with function mood_text(mood) as implicit;
create table tm (id int primary key, m mood not null);
insert into tm values (1, 'happy');
-- A composite type whose cast to text is a value even for a NULL, which a predicate over the cast
-- passes with no value in the column.
create type tagged as (v int);
create function tagged_text(tagged) returns text language sql as $$ select 'x'::text $$;
create cast (tagged as text) with function tagged_text(tagged) as implicit;
create table tt (id int primary key, g tagged);
insert into tt values (1, null), (2, row(2));
