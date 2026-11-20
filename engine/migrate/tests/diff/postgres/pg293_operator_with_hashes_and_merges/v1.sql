create function eq3(int, int) returns boolean language sql immutable as $$ select $1 = $2 $$;
