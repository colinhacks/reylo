select array(select c from (select x.id, l.rs from comments x left join (select array(select r from replies r) as rs) l on false) c) as arr
