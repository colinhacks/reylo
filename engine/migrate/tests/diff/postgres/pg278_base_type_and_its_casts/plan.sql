set check_function_bodies = off;
create function tag_of(bigint) returns tag language sql immutable as $$ select $1::text::tag $$;
create cast (bigint as tag) with function tag_of(bigint);
create cast (integer as tag) with inout;
reset check_function_bodies;
