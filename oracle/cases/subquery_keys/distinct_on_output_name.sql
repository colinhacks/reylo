select s.title, s.t2 from (select distinct on (title) author_id as title, title as t2 from posts order by title) s where s.t2 = :t
