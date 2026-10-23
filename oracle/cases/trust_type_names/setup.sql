-- A type named like an aggregate makes a call of one argument with no exact match a cast to it
-- (`func_get_detail`), which reads each row's value: `max(v)` of a varchar and `count(id)` of an
-- int are no aggregates here, a user's aggregate `max(bytea)` besides.
create table t (id int primary key, v varchar(10) not null);
insert into t values (1, 'a'), (2, 'b'), (3, 'c');
create domain max as varchar;
create function bmax_sfunc(bytea, bytea) returns bytea language sql immutable as $$ select $2 $$;
create aggregate max(bytea) (sfunc = bmax_sfunc, stype = bytea);
create domain count as text;
