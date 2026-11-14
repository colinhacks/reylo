select l.arr from (select u as c from users u) o, lateral (select array(select c from (select 1 as zip) c) as arr) l
