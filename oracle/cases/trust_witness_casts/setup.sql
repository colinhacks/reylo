-- A cast function of a user's from bool to int8, 1 for anything, NULL included. A strict WHERE
-- that rests on the cast does not prove the outer-joined row exists: a NULL-extended row passes.
create table t (id int primary key, flag bool, name text not null);
create table posts (id int primary key, author_id int, title text not null);
create function bool_to_int8_one(bool) returns int8 language sql immutable as $$ select 1::int8 $$;
create cast (bool as int8) with function bool_to_int8_one(bool);
