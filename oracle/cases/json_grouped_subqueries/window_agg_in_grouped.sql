select (select jsonb_build_object('w', jsonb_agg(p.title) over (), 'x', count(*)) from posts p group by p.title order by p.title limit 1) as j
