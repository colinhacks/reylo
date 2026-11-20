create function eq3(int, int) returns boolean language sql immutable as $$ select $1 = $2 $$;
create operator === (function = eq3, leftarg = int, rightarg = int, commutator = ===, hashes, merges, restrict = eqsel, join = eqjoinsel);
