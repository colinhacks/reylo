with a as (select jsonb_agg(title) as titles from posts) select titles from a
