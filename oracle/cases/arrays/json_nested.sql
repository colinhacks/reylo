select json_build_object('stacked', array_agg(cells order by id), 'pair', array[cells, cells]) as doc
from grids
where cardinality(cells) > 0
group by cells
