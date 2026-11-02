select s.x from (select distinct on (text) (p.author_id + 0)::text, p.text as x from (select author_id, title as text from posts) p order by text) s where s.x = :x
