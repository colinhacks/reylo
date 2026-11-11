select array(select c from (select distinct on (post_id) post_id, body from comments order by post_id, id desc) c) as arr
