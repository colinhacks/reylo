with w as (select array(select r from (select id from replies) r) as rs) select array(select c from (select x.id, w.rs from comments x left join w on false) c) as arr
