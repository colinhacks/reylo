select (array(select c from (select id from comments where false) c))[1] as first
