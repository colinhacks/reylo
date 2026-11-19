set check_function_bodies = off;
create type later;
create function joined(text, text) returns boolean language sql immutable as $$ select $1 = $2 $$;
create operator public.||++ (function = joined, leftarg = text, rightarg = text, commutator = operator(public.||+++));
reset check_function_bodies;
