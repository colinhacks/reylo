select d.arr from posts p cross join lateral (select array(select c from (select p.id + 1 as n) c) as arr) d right join users u on false
