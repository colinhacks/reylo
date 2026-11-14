select array(select d from (select x.id, array(select c from replies c) as rs from cc x) d) as arr
