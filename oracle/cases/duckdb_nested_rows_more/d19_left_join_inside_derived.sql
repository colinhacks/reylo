select array(select c from (select x.id, r.note from comments x left join replies r on r.comment_id = x.id) c) as arr
