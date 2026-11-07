select array(select c from (select 1 as n) c) as arr from (select {'n': null::integer} as c) o
