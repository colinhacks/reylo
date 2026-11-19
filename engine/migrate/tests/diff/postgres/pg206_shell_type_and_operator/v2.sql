create table t (id int);
create type later;
create function joined(text, text) returns boolean language sql immutable as $$ select $1 = $2 $$;
create operator ||++ (function = joined, leftarg = text, rightarg = text, commutator = ||+++);
