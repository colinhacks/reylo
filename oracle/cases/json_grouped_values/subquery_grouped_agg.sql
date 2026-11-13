select (select jsonb_build_object('name', u.name, 'titles', jsonb_agg(p.title), 'best', max(p.score)) from users u left join posts p on p.user_id = u.id group by u.id order by u.id limit 1) as j
