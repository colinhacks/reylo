select array(select c from (select unnest({'a': null::integer}), 1 as k) c) as arr
