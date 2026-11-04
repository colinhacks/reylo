select array_agg(cells order by id) as stacked, array[cells, cells] as pair, array[array[1, 2], array[3, null]] as literal, array(select g.cells from grids g order by g.id) as rows_of
from grids
where id = :id::int
group by cells
