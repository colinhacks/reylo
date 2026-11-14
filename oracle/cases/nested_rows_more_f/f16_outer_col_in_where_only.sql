select array(select c from (select 'z' as street) c where c is not null) as arr from cc
