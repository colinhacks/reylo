select l.* from posts p cross join lateral (select substring(p.title from 'zz+') as sub, nullif(p.id, p.id) as nul, (array[p.id])[2] as sub2, regexp_substr(p.title, 'q') as rs) l
