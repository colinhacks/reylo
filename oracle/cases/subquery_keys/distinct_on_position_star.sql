select s.a from (select distinct on (2) p.*, p.title as a from posts p order by 2) s where s.a = :a
