select array(select c from comments c) as arr from (select {'id': null::bigint} as c) o
