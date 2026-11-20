create type t__old as (a integer, b text);
alter table r alter column c type t__old using c::text::t__old;
alter type t drop attribute b;
alter type t add attribute d integer;
alter type t add attribute b text;
alter table r alter column c type t using case when num_nulls(c) = 1 then null else row((c).a::integer, null::integer, (c).b::text)::t end;
drop type t__old;
