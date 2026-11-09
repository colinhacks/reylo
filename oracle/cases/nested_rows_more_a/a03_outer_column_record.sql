select array(select c from (select 1 as id) c) as arr from (select row('x', null::int) as c) o
