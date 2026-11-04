select array[array[cells]] as cube, array_agg(array[id, id] order by id) as rows_agg, array(select array[g.id, g.id] from grids g order by g.id) as rows_built, array[array(select g.id from grids g order by g.id)] as wrapped, array(select 1) as scalars
from grids
group by cells
