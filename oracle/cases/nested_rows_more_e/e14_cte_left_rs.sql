with w as (select x.id, array(select r from replies r where r.comment_id = x.id) as rs from comments x) select w.rs from posts p left join w on false
