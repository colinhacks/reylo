select array[x] as one, array[y] as held, array[coalesce(x, y)] as either, array(select l.x from lists l order by l.id) as rows_of, json_build_object('a', array[x]) as doc from lists
