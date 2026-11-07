with comments as (select 1::bigint as id, 1::bigint as post_id, $$x$$::text as body, $$n$$::text as note) select note from notes_for(:p::int8)
