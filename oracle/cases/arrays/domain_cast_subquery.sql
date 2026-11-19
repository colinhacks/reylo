select array(select l.x::int[] from lists l where l.x is not null order by l.id) as rows_of
