select array(select c from (select x.id, l.rs from comments x full join (select array(select r from (select id from replies) r) as rs) l on false) c) as arr
