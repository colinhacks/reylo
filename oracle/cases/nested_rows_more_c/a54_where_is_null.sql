select array(select c from (select body from comments) c where c is null) as arr
