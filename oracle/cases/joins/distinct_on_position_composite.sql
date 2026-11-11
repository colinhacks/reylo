select distinct on (2) (p).*, p.title as t from posts p where p.title = :t order by 2
