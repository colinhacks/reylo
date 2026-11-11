with picked as (select id, name from teams where score > 0) select jsonb_agg(picked order by picked.id) as rows from picked
