select (select jsonb_build_object('w', jsonb_agg(p.id) over (order by p.id rows between current row and current row)) from posts p group by p.id order by p.id limit 1) as j
