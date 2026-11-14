select json_object('n', count(*), 'm', max(p.n)) as o, p.title as tag from posts p group by tag
