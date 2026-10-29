with x as (select 'from a query' as body) insert into notes (body) select body from x returning id, body, tag
