with d as (select json_object('t', p.title, 'c', count(*)) as o from posts p group by p.title with rollup) select d.o from d
