select (select jsonb_build_object('k', q.id, 't', q.title, 'c', count(*)) from posts p group by p.tag order by 1 limit 1) as j from users u left join posts q on q.user_id = u.id
