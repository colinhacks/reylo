with comments as (select 9::bigint as id, 9::bigint as post_id, $$z$$ as body) select id, body from comments_for(:p::int8)
