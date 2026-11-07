select array(select c from (select x.id, array(select r from (select id from replies limit 0) r) as rs from comments x) c) as arr
